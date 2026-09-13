"""Conservative single-output GDM/Wayland pilot guard. No display mutations.

Poll public logind and Mutter D-Bus APIs. Latch invalid on any observed target
change: operator must restart against the newly verified video source. This is
not automatic login/session handoff. The consumer also enforces file freshness.
"""
import argparse
import json
import os
import re
import signal
import tempfile
import time
import uuid

import dbus

parser = argparse.ArgumentParser()
parser.add_argument('--output', required=True)
parser.add_argument('--session', required=True)
parser.add_argument('--session-bus', required=True)
parser.add_argument('--session-uid', required=True, type=int)
parser.add_argument('--drm-state', required=True)
parser.add_argument('--width', type=int, default=1920)
parser.add_argument('--height', type=int, default=1080)
args = parser.parse_args()
if os.geteuid() != 0 or not os.path.isabs(args.output):
    raise RuntimeError('root and absolute output required')
parent = os.path.dirname(args.output)
metadata = os.stat(parent, follow_symlinks=False)
if metadata.st_uid != 0 or metadata.st_mode & 0o077:
    raise RuntimeError('guard directory must be root private')

running = True
def stop(*_):
    global running
    running = False
signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)

system = dbus.SystemBus()
seat = dbus.Interface(system.get_object('org.freedesktop.login1', '/org/freedesktop/login1/seat/seat0'),
                      'org.freedesktop.DBus.Properties')
# Authenticate this private session-bus connection as its actual owner, then
# restore root for the root-owned publication file and DRM debug state reads.
os.seteuid(args.session_uid)
try:
    session_bus = dbus.bus.BusConnection(args.session_bus)
finally:
    os.seteuid(0)
display = dbus.Interface(session_bus.get_object('org.gnome.Mutter.DisplayConfig',
                         '/org/gnome/Mutter/DisplayConfig'), 'org.gnome.Mutter.DisplayConfig')

def snapshot():
    active = seat.Get('org.freedesktop.login1.Seat', 'ActiveSession', timeout=0.2)
    if str(active[0]) != args.session:
        raise RuntimeError('active session changed')
    session = dbus.Interface(system.get_object('org.freedesktop.login1', str(active[1])),
                             'org.freedesktop.DBus.Properties')
    locked = bool(session.Get('org.freedesktop.login1.Session', 'LockedHint', timeout=0.2))
    serial, monitors, logical, _ = display.GetCurrentState(timeout=0.2)
    if len(logical) != 1:
        raise RuntimeError('single-output mapping required')
    x, y, scale, transform, primary, outputs, _ = logical[0]
    if int(x) != 0 or int(y) != 0 or float(scale) != 1 or int(transform) != 0 or len(outputs) != 1:
        raise RuntimeError('unsupported output mapping')
    connector = str(outputs[0][0])
    current = []
    for spec, modes, _ in monitors:
        if str(spec[0]) == connector:
            current = [mode for mode in modes if mode[6].get('is-current', False)]
    # The virtual monitor has a native capture mode (normally 1920x1080),
    # while GNOME may select a different logical mode for the user session.
    # Do not force or reject that choice; input coordinates are mapped against
    # the rendered capture and the active DRM plane remains the invariant.
    if len(current) != 1:
        raise RuntimeError('compositor mode unavailable')
    with open(args.drm_state) as source:
        drm = source.read(65536)
    crtcs = re.findall(r'^crtc\[\d+\]:.*?(?=^\w|\Z)', drm, re.M | re.S)
    active_crtcs = [block for block in crtcs if re.search(r'^\s+active=1$', block, re.M)]
    if len(active_crtcs) != 1 or f'mode: "{args.width}x{args.height}"' not in active_crtcs[0]:
        raise RuntimeError('DRM output inactive or changed')
    return (str(active[0]), locked, int(serial), connector, str(current[0][0]))

def publish(valid, identity):
    fd, temporary = tempfile.mkstemp(prefix='.guard-', dir=parent)
    try:
        with os.fdopen(fd, 'w') as file:
            json.dump({'session': identity, 'width': args.width, 'height': args.height, 'valid': valid}, file)
        os.replace(temporary, args.output)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)

identity = args.session + ':' + uuid.uuid4().hex
baseline = None
valid = True
try:
    baseline = snapshot()
    print('input target guard validated single 1:1 output', flush=True)
    while running:
        started = time.monotonic()
        try:
            if snapshot() != baseline:
                valid = False
        except Exception:
            valid = False
        publish(valid, identity)
        if not valid:
            print('input target changed or unavailable; control stays disabled until revalidated', flush=True)
            break
        time.sleep(max(0, 0.1 - (time.monotonic() - started)))
finally:
    publish(False, identity)
