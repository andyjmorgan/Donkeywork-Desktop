"""Read ONLY our virtual devices; assert releases after disconnect/lease expiry.

Run as root on the approved single-output pilot. Does not grab devices.
Presses Shift and left button at a caller-selected harmless blank coordinate.
"""
import argparse
import glob
import json
import os
import select
import socket
import struct
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--socket', required=True)
parser.add_argument('--sysfs', action='append', required=True)
parser.add_argument('--x', type=int, required=True)
parser.add_argument('--y', type=int, required=True)
parser.add_argument('--freeze-guard', action='store_true', help='Temporarily SIGSTOP only dwconsole-input-guard.service')
args = parser.parse_args()
event = struct.Struct('@llHHi')
fds = []
for path in args.sysfs:
    if not path.startswith('/sys/devices/virtual/input/input'):
        raise RuntimeError('only explicit virtual input nodes allowed')
    with open(path + '/name') as name:
        if name.read().strip() not in ('DonkeyWork Console Keyboard', 'DonkeyWork Console Pointer'):
            raise RuntimeError('not our virtual device')
    for node in glob.glob(path + '/event*'):
        fds.append(os.open('/dev/input/' + os.path.basename(node), os.O_RDONLY | os.O_NONBLOCK))
if len(fds) != 2:
    raise RuntimeError('expected exactly our keyboard and pointer event nodes')

def receive(sock):
    def exact(n):
        result = b''
        while len(result) < n:
            part = sock.recv(n - len(result))
            if not part:
                raise RuntimeError('unexpected EOF')
            result += part
        return result
    length, = struct.unpack('!I', exact(4))
    if not 0 < length <= 4096:
        raise RuntimeError('bad frame')
    return json.loads(exact(length))

try:
    modes = ('disconnect', 'expire', 'traffic_without_renew') + (('guard_stale',) if args.freeze_guard else ())
    for mode in modes:
        sock = socket.socket(socket.AF_UNIX)
        sock.settimeout(2)
        sock.connect(args.socket)
        acquire = b'{"type":"acquire"}'
        sock.sendall(struct.pack('!I', len(acquire)) + acquire)
        hello = receive(sock)
        assert hello.get('type') == 'ready', hello
        for seq, payload in enumerate((
            {'type': 'key', 'hid': 225, 'down': True},
            {'type': 'button', 'button': 1, 'down': True, 'x': args.x, 'y': args.y},
        ), 1):
            data = json.dumps({'generation': hello['generation'], 'sequence': seq, 'event': payload}).encode()
            sock.sendall(struct.pack('!I', len(data)) + data)
            assert receive(sock) == {'type': 'ack', 'sequence': seq, 'accepted': True}
        started = time.monotonic()
        if mode == 'disconnect':
            sock.close()
        if mode == 'guard_stale':
            subprocess.run(['systemctl', 'kill', '--kill-whom=main', '--signal=SIGSTOP', 'dwconsole-input-guard.service'], check=True)
        seen = set()
        deadline = started + 1.6
        release_times = []
        next_move = started + 0.1
        sequence = 2
        transport_open = True
        while time.monotonic() < deadline:
            if mode in ('traffic_without_renew', 'guard_stale') and transport_open and time.monotonic() >= next_move:
                sequence += 1
                data = json.dumps({'generation': hello['generation'], 'sequence': sequence,
                                   'event': ({'type': 'renew'} if mode == 'guard_stale' else
                                             {'type': 'move', 'x': args.x, 'y': args.y})}).encode()
                try:
                    sock.sendall(struct.pack('!I', len(data)) + data)
                    receive(sock)
                except (OSError, RuntimeError):
                    transport_open = False
                next_move = time.monotonic() + 0.1
            ready, _, _ = select.select(fds, [], [], min(0.05, max(0, deadline - time.monotonic())))
            for fd in ready:
                data = os.read(fd, event.size * 64)
                for offset in range(0, len(data), event.size):
                    _, _, kind, code, value = event.unpack_from(data, offset)
                    if kind == 1 and code in (42, 272):
                        seen.add((code, value))
                        if value == 0:
                            release_times.append(time.monotonic() - started)
            if all((code, val) in seen for code in (42, 272) for val in (0, 1)):
                break
        sock.close()
        assert all((code, val) in seen for code in (42, 272) for val in (0, 1)), 'missing down/up'
        print(json.dumps({'mode': mode, 'bothReleased': True, 'latestObservedReleaseMs': round(max(release_times) * 1000)}))
finally:
    if args.freeze_guard:
        subprocess.run(['systemctl', 'kill', '--kill-whom=main', '--signal=SIGCONT', 'dwconsole-input-guard.service'], check=False)
    for fd in fds:
        os.close(fd)
