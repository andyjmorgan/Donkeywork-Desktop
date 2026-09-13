"""Observe only our virtual keyboard; report booleans, never key payloads."""
import argparse
import glob
import json
import os
import select
import struct
import time

parser = argparse.ArgumentParser()
parser.add_argument('--sysfs', required=True)
args = parser.parse_args()
if not args.sysfs.startswith('/sys/devices/virtual/input/input'):
    raise RuntimeError('explicit virtual input node required')
with open(args.sysfs + '/name') as name:
    if name.read().strip() != 'DonkeyWork Console Keyboard':
        raise RuntimeError('not our keyboard')
nodes = glob.glob(args.sysfs + '/event*')
if len(nodes) != 1:
    raise RuntimeError('one event node required')
fd = os.open('/dev/input/' + os.path.basename(nodes[0]), os.O_RDONLY | os.O_NONBLOCK)
packet = struct.Struct('@llHHi')
down = False
released = False
try:
    print('observer ready', flush=True)
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        ready, _, _ = select.select([fd], [], [], max(0, deadline - time.monotonic()))
        if not ready:
            break
        data = os.read(fd, packet.size * 64)
        for offset in range(0, len(data), packet.size):
            _, _, kind, code, value = packet.unpack_from(data, offset)
            # Only the known test modifier; discard all other key data.
            if kind == 1 and code == 42:
                if value == 1:
                    down = True
                if value == 0 and down:
                    released = True
        if released:
            break
    print(json.dumps({'injectedModifierObserved': down, 'releaseObserved': released}))
    if not released:
        raise RuntimeError('expected injected modifier release not observed')
finally:
    os.close(fd)
