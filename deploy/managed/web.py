#!/usr/bin/env python3
"""Lab-only adapter: existing WebRTC bridge to a private managed X11 core."""
import argparse
import json
import os
from pathlib import Path
import select
import signal
import socket
import struct
import subprocess
import threading
import time
import uuid


def read_exact(conn, n):
    data = bytearray()
    while len(data) < n:
        part = conn.recv(n - len(data))
        if not part:
            raise EOFError()
        data.extend(part)
    return bytes(data)


def read_json(conn, limit=65536):
    n, = struct.unpack("!I", read_exact(conn, 4))
    if not 0 < n <= limit:
        raise ValueError("record limit")
    return json.loads(read_exact(conn, n))


def write_record(conn, data):
    conn.sendall(struct.pack("!I", len(data)) + data)


def write_json(conn, value):
    write_record(conn, json.dumps(value).encode())


class Core:
    def __init__(self, root):
        path = root / "cli.sock"
        for item in (root, path):
            st = item.stat()
            if st.st_uid != os.getuid() or st.st_mode & 0o022:
                raise ValueError("unsafe core endpoint")
        self.conn = socket.socket(socket.AF_UNIX)
        self.conn.settimeout(3)
        self.conn.connect(str(path))
        _, uid, _ = struct.unpack("3i", self.conn.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
        if uid != os.getuid():
            self.conn.close()
            raise ValueError("core UID mismatch")
        self.bound = {}

    def call(self, kind, payload=None):
        mid = str(uuid.uuid4())
        write_json(self.conn, dict(protocol="dwdesktop.local", version="0.2.0", type=kind,
                                  messageId=mid, payload=payload or {}, **self.bound))
        reply = read_json(self.conn)
        if reply.get("protocol") != "dwdesktop.local" or reply.get("version") != "0.2.0" or reply.get("requestMessageId") != mid:
            raise ValueError("core correlation")
        if reply["type"] == "error":
            raise ValueError("core rejected operation")
        if self.bound and any(reply.get(k) != v for k, v in self.bound.items()):
            raise ValueError("core session mismatch")
        if kind == "session.open":
            self.bound = {k: reply[k] for k in ("sessionId", "sessionEpoch")}
        if reply["type"] == "screenshot.result":
            n = reply["payload"]["payloadBytes"]
            if not 0 < n <= 67108864:
                raise ValueError("image limit")
            while n:
                chunk = read_exact(self.conn, min(n, 65536))
                n -= len(chunk)
        return reply["payload"]

    def close(self):
        try:
            if self.bound:
                self.call("session.close")
        except (OSError, EOFError, ValueError):
            pass
        self.conn.close()


def topology(root):
    c = Core(root)
    try:
        displays = c.call("describe")["displays"]
        if len(displays) != 1:
            raise ValueError("managed pilot requires one display")
        return displays[0]
    finally:
        c.close()


def input_loop(conn, root, display):
    core = None
    lease = None
    seq = 0
    deadline = 0
    snapshot_at = 0
    snapshot = None
    def release():
        nonlocal core, lease
        if core:
            core.close()
        core = None
        lease = None
    def capture():
        nonlocal snapshot, snapshot_at
        snapshot = core.call("screenshot.request", {"displayId": display["displayId"], "includeCursor": False})
        if any(snapshot[k] != display[k] for k in ("width", "height", "topologyRevision")):
            raise ValueError("display changed")
        snapshot_at = time.monotonic()
    try:
        while True:
            ready, _, _ = select.select([conn], [], [], max(0, deadline - time.monotonic()) if core else None)
            if not ready:
                release()
                write_json(conn, {"type": "unavailable", "reason": "control expired"})
                continue
            conn.settimeout(3)
            message = read_json(conn, 4096)
            try:
                if message.get("type") == "acquire":
                    release()
                    core = Core(root)
                    core.call("session.open")
                    capture()
                    lease = core.call("control.acquire")["leaseId"]
                    generation = str(uuid.uuid4())
                    seq = 0
                    deadline = time.monotonic() + 1
                    write_json(conn, dict(type="ready", protocol="dwconsole.input", version="0.2.0",
                                         generation=generation, width=display["width"], height=display["height"], leaseMs=1000))
                    continue
                if not core or time.monotonic() >= deadline or message.get("generation") != generation or message.get("sequence") != seq + 1:
                    raise ValueError("stale input")
                seq += 1
                event = message["event"]
                kind = event["type"]
                if kind == "release":
                    release()
                elif kind == "renew":
                    core.call("control.renew", {"leaseId": lease})
                    if time.monotonic() - snapshot_at > 60:
                        capture()
                    deadline = time.monotonic() + 1
                else:
                    # Core input sequencing excludes renew/release messages.
                    sequence = getattr(core, "sequence", 0) + 1
                    core.sequence = sequence
                    payload = dict(leaseId=lease, inputSequence=sequence)
                    if kind == "reset":
                        core.call("input.reset", payload)
                    else:
                        payload.update(snapshotId=snapshot["snapshotId"], displayId=display["displayId"],
                                       topologyRevision=snapshot["topologyRevision"])
                        if kind in ("move", "button"):
                            payload.update(x=event["x"], y=event["y"], action="move", button="none")
                            if kind == "button":
                                payload.update(action="down" if event["down"] else "up",
                                               button={1: "left", 2: "right", 3: "middle"}[event["button"]])
                            core.call("input.pointer", payload)
                        elif kind == "key":
                            payload.update(usage=event["hid"], action="down" if event["down"] else "up")
                            core.call("input.key", payload)
                        elif kind == "wheel":
                            ticks = []
                            for axis, positive, negative in (("vertical", "wheel_down", "wheel_up"),
                                                             ("horizontal", "wheel_right", "wheel_left")):
                                amount = event[axis]
                                if type(amount) is not int or abs(amount) > 32:
                                    raise ValueError("wheel bound")
                                ticks.extend([positive if amount > 0 else negative] * abs(amount))
                            if not ticks:
                                raise ValueError("empty wheel")
                            for index, button in enumerate(ticks):
                                if index:
                                    core.sequence += 1
                                payload.update(x=event["x"], y=event["y"], action="click", button=button,
                                               inputSequence=core.sequence)
                                core.call("input.pointer", payload)
                        else:
                            raise ValueError("unsupported input event")
                write_json(conn, {"type": "ack", "sequence": seq, "accepted": True})
            except (ValueError, KeyError, OSError, EOFError):
                release()
                reply = {"type": "unavailable", "reason": "managed input unavailable"}
                if "requestId" in message:
                    reply["requestId"] = message["requestId"]
                write_json(conn, reply)
    except (OSError, EOFError, ValueError):
        pass
    finally:
        release()
        conn.close()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("--host", choices=["192.168.69.28", "192.168.69.21", "192.168.69.17"])
    p.add_argument("--resize", nargs=2, type=int)
    p.add_argument("--http-port", type=int, default=8095)
    a = p.parse_args()
    root = a.root.resolve()
    instance = json.loads((root / "instance.json").read_text()) if (root / "instance.json").exists() else {}
    display_name = ':' + str(instance.get("display", 109))
    if a.resize:
        c = Core(root)
        try:
            d = c.call("describe")["displays"][0]
            c.call("session.open")
            lease = c.call("control.acquire")["leaseId"]
            result = c.call("display.resize", dict(leaseId=lease, displayId=d["displayId"],
                topologyRevision=d["topologyRevision"], width=a.resize[0], height=a.resize[1]))
            print(json.dumps(result))
        finally:
            c.close()
        return
    if not a.host:
        p.error("--host required for streaming")
    os.environ.update(DISPLAY=display_name, XAUTHORITY=str(root / "Xauthority"))
    stopped = threading.Event()
    signal.signal(signal.SIGTERM, lambda *_: stopped.set())
    signal.signal(signal.SIGINT, lambda *_: stopped.set())
    while not stopped.is_set():
        display = topology(root)
        media_parent, media_child = socket.socketpair()
        input_parent, input_child = socket.socketpair()
        bridge = subprocess.Popen([str(root / "bin/dwconsole-web"), "--listen", a.host + ":" + str(a.http_port),
            "--ice-listen", a.host + ":" + str(instance.get("icePort", 8096)), "--origin", "http://" + a.host + ":8095",
            "--stream-fd", str(media_child.fileno()), "--input-fd", str(input_child.fileno()),
            "--assets", str(root / "web-assets"), "--managed-root", str(root)], pass_fds=[media_child.fileno(), input_child.fileno()])
        media_child.close()
        input_child.close()
        worker = threading.Thread(target=input_loop, args=(input_parent, root, display), daemon=True)
        worker.start()
        header = dict(protocol="dwconsole.stream", version="0.1.0", codec="h264", bitstream="annex-b",
                      encoder="libx264", displayId=display["displayId"], device="managed-x11" + display_name,
                      width=display["width"], height=display["height"], framesPerSecond=30, pixelFormat="yuv420p")
        media_parent.settimeout(3)
        write_json(media_parent, header)
        encoder = subprocess.Popen(["ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-f", "x11grab",
            "-framerate", "30", "-video_size", f'{display["width"]}x{display["height"]}', "-i", display_name,
            "-an", "-c:v", "libx264", "-preset", "ultrafast", "-tune", "zerolatency", "-threads", "4",
            "-pix_fmt", "yuv420p", "-g", "30", "-bf", "0", "-x264-params", "repeat-headers=1:aud=1",
            "-f", "h264", "pipe:1"], stdout=subprocess.PIPE)
        next_check = time.monotonic() + 1
        try:
            while not stopped.is_set() and bridge.poll() is None and encoder.poll() is None and worker.is_alive():
                if time.monotonic() >= next_check:
                    now = topology(root)
                    if any(now[k] != display[k] for k in ("width", "height", "topologyRevision")):
                        break
                    next_check = time.monotonic() + 1
                ready, _, _ = select.select([encoder.stdout], [], [], .2)
                if ready:
                    data = os.read(encoder.stdout.fileno(), 65536)
                    if not data:
                        break
                    write_record(media_parent, data)
        except (BrokenPipeError, ConnectionResetError):
            # Recreate the connected adapters, not the desktop, after bridge loss.
            pass
        finally:
            media_parent.close()
            try:
                input_parent.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            worker.join(timeout=4)
            for proc in (encoder, bridge):
                proc.terminate()
            for proc in (encoder, bridge):
                try:
                    proc.wait(timeout=4)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()
        stopped.wait(.25)


if __name__ == "__main__":
    main()
