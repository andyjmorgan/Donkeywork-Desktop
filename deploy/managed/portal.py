#!/usr/bin/env python3
"""Persistent lab viewer portal; only an explicit POST may start a desktop."""
import argparse
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import signal
import subprocess
import threading
import time
import urllib.request
import urllib.error
from web import topology

p = argparse.ArgumentParser()
p.add_argument("root", type=Path)
p.add_argument("--host", required=True)
a = p.parse_args()
root = a.root.resolve()
origin = f"http://{a.host}:8095"
lock = threading.Lock()
stop = threading.Event()
child = None


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(root / "web-assets"), **kwargs)
    def log_message(self, *_):
        pass
    def result(self, code, data):
        body = json.dumps(data).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)
    def allowed(self):
        return self.headers.get("Host") == f"{a.host}:8095" and self.headers.get("Origin", origin) == origin
    def proxy(self, body=None):
        request = urllib.request.Request(f"http://{a.host}:8097" + self.path, data=body,
            headers={"Host": f"{a.host}:8095", "Origin": origin, "Content-Type": "application/json"}, method=self.command)
        try:
            with urllib.request.urlopen(request, timeout=12) as response:
                data = response.read(1048576)
                self.send_response(response.status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.send_header("Cache-Control", "no-store")
                self.end_headers()
                self.wfile.write(data)
        except urllib.error.HTTPError as error:
            self.result(error.code, {"error": "worker rejected request"})
        except (OSError, urllib.error.URLError):
            self.result(503, {"error": "viewer starting"})
    def do_GET(self):
        if not self.allowed():
            return self.result(403, {"error": "origin rejected"})
        if self.path == "/api/status":
            try:
                topology(root)
            except (OSError, ValueError, EOFError):
                return self.result(200, {"state": "session_ended"})
            return self.proxy()
        if self.path.startswith("/api/"):
            return self.result(404, {})
        return super().do_GET()
    def do_POST(self):
        if not self.allowed() or self.headers.get("Origin") != origin:
            return self.result(403, {"error": "origin required"})
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if not 0 <= length <= 262144:
                raise ValueError()
        except ValueError:
            return self.result(400, {})
        body = self.rfile.read(length)
        if self.path == "/api/session/connect":
            if body != b"{}":
                return self.result(400, {})
            if not lock.acquire(blocking=False):
                return self.result(409, {"error": "start in progress"})
            try:
                try:
                    topology(root)
                    return self.result(200, {"state": "existing"})
                except (OSError, ValueError, EOFError):
                    pass
                # Reload after retiring an older transient pilot unit so systemd
                # resolves the persistent, non-autostart desktop definition.
                subprocess.run(["systemctl", "--user", "daemon-reload"], check=True, timeout=10)
                start = subprocess.run(["systemctl", "--user", "start", "dwdesktop-managed-poc.service"],
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15)
                if start.returncode:
                    return self.result(503, {"error": "desktop start failed"})
                for _ in range(30):
                    try:
                        topology(root)
                        return self.result(200, {"state": "created"})
                    except (OSError, ValueError, EOFError):
                        time.sleep(.5)
                return self.result(503, {"error": "desktop not ready"})
            finally:
                lock.release()
        if self.path in ("/api/offer", "/api/resize"):
            return self.proxy(body)
        return self.result(404, {})


def monitor():
    global child
    while not stop.wait(1):
        try:
            topology(root)
            if child is None or child.poll() is not None:
                child = subprocess.Popen(["python3", str(root / "web.py"), str(root), "--host", a.host, "--http-port", "8097"])
        except (OSError, EOFError, ValueError):
            if child and child.poll() is None:
                child.terminate()
                try:
                    child.wait(5)
                except subprocess.TimeoutExpired:
                    child.kill()
            child = None


server = ThreadingHTTPServer((a.host, 8095), Handler)
server.timeout = .5
signal.signal(signal.SIGTERM, lambda *_: stop.set())
signal.signal(signal.SIGINT, lambda *_: stop.set())
threading.Thread(target=monitor, daemon=True).start()
try:
    while not stop.is_set():
        server.handle_request()
finally:
    server.server_close()
    if child and child.poll() is None:
        child.terminate()
