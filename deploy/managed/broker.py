#!/usr/bin/env python3
"""Single-host lab broker with independent managed desktops, not fleet auth."""
import argparse
import configparser
import http.client
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import pwd
import re
import shutil
import signal
import socket
import socketserver
import subprocess
import threading
import time
import urllib.request
import urllib.error
import uuid
from web import topology


class Broker:
    def __init__(self, root, host):
        self.root, self.host = root, host
        self.instances = root / "sessions"
        self.instances.mkdir(exist_ok=True, mode=0o700)
        self.lock = threading.Lock()
        self.children = {}
        self.stop = threading.Event()

    def console(self, method, path, body=None):
        config = self.root / 'console-socket'
        if not config.exists():
            return None
        endpoint = Path(config.read_text().strip())
        for entry in (config, endpoint.parent, endpoint):
            st = entry.stat()
            if st.st_uid != os.getuid() or st.st_mode & 0o022:
                raise ValueError('unsafe console endpoint')
        conn = http.client.HTTPConnection('localhost', timeout=10)
        peer = socket.socket(socket.AF_UNIX)
        peer.settimeout(10)
        try:
            peer.connect(str(endpoint))
            import struct
            _, uid, _ = struct.unpack('3i', peer.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
            if uid != os.getuid():
                raise ValueError('console peer mismatch')
            conn.sock = peer
            conn.request(method, path, body=body, headers={'Origin': 'http://localhost', 'Content-Type': 'application/json'})
            response = conn.getresponse()
            return response.status, json.loads(response.read(524288))
        finally:
            peer.close()
            conn.close()

    def console_inventory(self):
        try:
            response = self.console('GET', '/api/desktops')
            return response[1]['desktops'] if response and response[0] == 200 else []
        except (OSError, ValueError, KeyError, http.client.HTTPException):
            return []

    def environments(self):
        installed = []
        for directory in (Path('/usr/share/xsessions'), Path('/usr/share/wayland-sessions')):
            for entry in sorted(directory.glob('*.desktop')):
                parser = configparser.ConfigParser(interpolation=None, strict=False)
                try:
                    parser.read(entry)
                    name = parser['Desktop Entry'].get('Name', entry.stem)
                    installed.append(dict(id=entry.stem, name=name, type='x11' if directory.name == 'xsessions' else 'wayland'))
                except (configparser.Error, KeyError):
                    continue
        names = {item['id'] for item in installed if item['type'] == 'x11'}
        validated = ['xfce']
        if (self.root / 'validated-environments.json').exists():
            validated = json.loads((self.root / 'validated-environments.json').read_text())
        profiles = []
        for key, name, available in [('gnome', 'GNOME (Ubuntu)', 'ubuntu' in names and shutil.which('gnome-session')),
                                     ('xfce', 'Xfce', 'xfce' in names and shutil.which('xfce4-session'))]:
            if available:
                profiles.append(dict(id=key, name=name, installed=True, supported=True,
                                     validation='validated' if key in validated else 'pending'))
        return dict(environments=profiles, installedSessions=installed)

    def metadata(self, identifier):
        if not re.fullmatch('[0-9a-f]{32}', identifier):
            raise KeyError('unknown desktop')
        return json.loads((self.instances / identifier / 'instance.json').read_text())

    def state(self, item):
        unit = 'dwdesktop-session-' + item['id'] + '.service'
        state = subprocess.run(['systemctl', '--user', 'show', unit, '-p', 'ActiveState', '--value'],
                               capture_output=True, text=True, timeout=5).stdout.strip()
        if state not in ('active', 'activating', 'deactivating'):
            lifecycle = self.instances / item['id'] / 'lifecycle.json'
            if lifecycle.exists() and json.loads(lifecycle.read_text()).get('state') == 'failed':
                return 'failed'
            return 'failed' if state == 'failed' else 'ended'
        if state == 'deactivating':
            return 'closing'
        try:
            topology(self.instances / item['id'])
            return 'ready'
        except (OSError, ValueError, EOFError):
            return 'starting'

    def list(self):
        desktops, history = [], []
        for path in sorted(self.instances.glob('*/instance.json')):
            item = json.loads(path.read_text())
            state = self.state(item)
            record = {key: item[key] for key in ('id', 'environment', 'user', 'createdAt')}
            record['state'] = state
            (desktops if state in ('ready', 'starting', 'closing') else history).append(record)
        return dict(desktops=desktops, history=history[-20:])

    def create(self, environment, request_id):
        if environment not in {e['id'] for e in self.environments()['environments']}:
            raise ValueError('desktop environment is not supported on this host')
        if not re.fullmatch('[0-9a-f-]{36}', request_id):
            raise ValueError('invalid request ID')
        with self.lock:
            for path in self.instances.glob('*/instance.json'):
                previous = json.loads(path.read_text())
                if previous['requestId'] == request_id:
                    if previous['environment'] != environment:
                        raise ValueError('request ID already used with different environment')
                    return previous
            live = self.list()['desktops']
            if len(live) >= 4:
                raise ValueError('four-desktop pilot limit reached')
            occupied = {self.metadata(d['id'])['display'] for d in live}
            display = next((n for n in range(110, 126) if n not in occupied and
                            not Path(f'/tmp/.X11-unix/X{n}').exists() and not Path(f'/tmp/.X{n}-lock').exists()), None)
            if display is None:
                raise ValueError('no private display available')
            identifier = uuid.uuid4().hex
            target = self.instances / identifier
            target.mkdir(mode=0o700)
            item = dict(id=identifier, environment=environment, display=display, httpPort=8100 + display - 110,
                        icePort=8200 + display - 110, user=pwd.getpwuid(os.getuid()).pw_name,
                        createdAt=time.time(), requestId=request_id)
            (target / 'instance.json').write_text(json.dumps(item))
            for name in ('bin', 'web-assets'):
                (target / name).symlink_to(self.root / name, target_is_directory=True)
            for name in ('web.py', 'runtime.py', 'xorg.conf'):
                shutil.copyfile(self.root / name, target / name)
            unit = 'dwdesktop-session-' + identifier
            command = ['systemd-run', '--user', '--unit=' + unit, '--collect', '--property=Restart=no',
                       '--property=KillMode=control-group', '--property=TimeoutStopSec=10',
                       'python3', str(target / 'runtime.py'), str(target)]
            result = subprocess.run(command, capture_output=True, timeout=10)
            if result.returncode:
                raise RuntimeError('desktop service could not start')
            return item

    def monitor(self):
        while not self.stop.wait(1):
            try:
                live = {d['id']: d for d in self.list()['desktops']}
                for identifier, proc in list(self.children.items()):
                    if identifier not in live or live[identifier]['state'] != 'ready':
                        proc.terminate()
                        try:
                            proc.wait(6)
                        except subprocess.TimeoutExpired:
                            proc.kill()
                            proc.wait()
                        del self.children[identifier]
                for identifier, desktop in live.items():
                    if desktop['state'] != 'ready':
                        continue
                    proc = self.children.get(identifier)
                    if proc is None or proc.poll() is not None:
                        item = self.metadata(identifier)
                        target = self.instances / identifier
                        self.children[identifier] = subprocess.Popen(['python3', str(target / 'web.py'), str(target),
                            '--host', self.host, '--http-port', str(item['httpPort'])])
            except (OSError, ValueError, subprocess.SubprocessError):
                # An individual lifecycle transition must not kill inventory.
                continue


def serve(root, host, socket_path=None):
    broker = Broker(root, host)
    origin = f'http://{host}:8095'
    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(root / 'web-assets'), **kwargs)
        def log_message(self, *_): pass
        def result(self, code, data):
            body = json.dumps(data).encode()
            self.send_response(code)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(body)))
            self.send_header('Cache-Control', 'no-store')
            self.end_headers()
            self.wfile.write(body)
        def valid(self):
            if isinstance(self.server, socketserver.UnixStreamServer):
                return self.headers.get('Host') == 'localhost' and self.headers.get('Origin', 'http://localhost') == 'http://localhost'
            return self.headers.get('Host') == f'{host}:8095' and self.headers.get('Origin', origin) == origin
        def route(self, body=None):
            try:
                expected_origin = 'http://localhost' if isinstance(self.server, socketserver.UnixStreamServer) else origin
                if not self.valid() or (self.command == 'POST' and self.headers.get('Origin') != expected_origin):
                    return self.result(403, {'error': 'origin rejected'})
                if self.path == '/api/desktops' and self.command == 'GET':
                    inventory = broker.list()
                    inventory['desktops'].extend(broker.console_inventory())
                    return self.result(200, inventory)
                if self.path == '/api/environments' and self.command == 'GET':
                    return self.result(200, broker.environments())
                if self.path == '/api/desktops' and self.command == 'POST':
                    data = json.loads(body)
                    if set(data) != {'environment', 'requestId'}:
                        raise ValueError('invalid create request')
                    item = broker.create(data['environment'], data['requestId'])
                    return self.result(202, {'desktop': {k: item[k] for k in ('id', 'environment', 'user', 'createdAt')}, 'state': 'starting'})
                match = re.fullmatch(r'/api/desktops/([0-9a-f]{32})/(status|offer|resize|reconnect|close)', self.path)
                if match:
                    identifier, action = match.groups()
                    if any(d.get('id') == identifier for d in broker.console_inventory()):
                        result = broker.console(self.command, self.path, body)
                        return self.result(*result)
                    item = broker.metadata(identifier)
                    state = broker.state(item)
                    if action == 'status' and self.command == 'GET' and state != 'ready':
                        return self.result(200, {'state': 'session_ended'} if state in ('ended', 'failed') else {'state': 'session_starting'})
                    if action == 'reconnect' and self.command == 'POST':
                        if state != 'ready': return self.result(409, {'error': 'desktop is not ready'})
                        return self.result(200, {'id': identifier, 'url': '/?managed&desktop=' + identifier})
                    if action == 'close' and self.command == 'POST':
                        subprocess.run(['systemctl', '--user', 'stop', 'dwdesktop-session-' + identifier + '.service'], check=True, timeout=15)
                        return self.result(200, {'state': 'ended'})
                    if (action == 'status' and self.command == 'GET') or (action in ('offer', 'resize') and self.command == 'POST'):
                        request = urllib.request.Request(f'http://{host}:{item["httpPort"]}/api/{action}', data=body,
                            headers={'Host': f'{host}:8095', 'Origin': origin, 'Content-Type': 'application/json'}, method=self.command)
                        with urllib.request.urlopen(request, timeout=12) as response:
                            return self.result(response.status, json.loads(response.read(1048576)))
                return self.result(404, {'error': 'not found'})
            except (KeyError, FileNotFoundError): return self.result(404, {'error': 'desktop not found'})
            except (ValueError, TypeError): return self.result(400, {'error': 'invalid or unavailable desktop request'})
            except urllib.error.HTTPError as error: return self.result(error.code, {'error': 'desktop operation rejected'})
            except (OSError, RuntimeError, subprocess.SubprocessError): return self.result(503, {'error': 'desktop temporarily unavailable'})
        def do_GET(self):
            if self.path.startswith('/api/'): return self.route()
            if not self.valid(): return self.result(403, {})
            return super().do_GET()
        def do_POST(self):
            try:
                n = int(self.headers.get('Content-Length', '0'))
                if not 0 < n <= 262144: raise ValueError()
            except ValueError: return self.result(400, {})
            return self.route(self.rfile.read(n))
    server = ThreadingHTTPServer((host, 8095), Handler)
    unix_server = None
    if socket_path:
        class UnixHTTPServer(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
            daemon_threads = True
        if socket_path.exists():
            if not socket_path.is_socket():
                raise RuntimeError('refusing to replace non-socket broker path')
            probe = socket.socket(socket.AF_UNIX)
            try:
                probe.connect(str(socket_path))
            except ConnectionRefusedError:
                socket_path.unlink()
            else:
                raise RuntimeError('broker socket already active')
            finally:
                probe.close()
        unix_server = UnixHTTPServer(str(socket_path), Handler)
        socket_path.chmod(0o660)
        threading.Thread(target=unix_server.serve_forever, daemon=True).start()
    server.timeout = .5
    signal.signal(signal.SIGTERM, lambda *_: broker.stop.set())
    signal.signal(signal.SIGINT, lambda *_: broker.stop.set())
    threading.Thread(target=broker.monitor, daemon=True).start()
    try:
        while not broker.stop.is_set(): server.handle_request()
    finally:
        server.server_close()
        if unix_server:
            unix_server.shutdown()
            unix_server.server_close()
            socket_path.unlink(missing_ok=True)
        for proc in broker.children.values(): proc.terminate()


if __name__ == '__main__':
    os.umask(0o077)
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    parser.add_argument('--host', required=True, choices=['192.168.69.21', '192.168.69.28', '192.168.69.17'])
    parser.add_argument('--socket', type=Path)
    args = parser.parse_args()
    serve(args.root.resolve(), args.host, args.socket)
