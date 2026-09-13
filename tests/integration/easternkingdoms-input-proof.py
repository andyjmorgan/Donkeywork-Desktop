"""Bounded EK package input proof: CLI motion and standalone Shift only.

Never types text, clicks, changes output modes, restarts services or logs out.
Observes only DonkeyWork-owned virtual devices; Xlib queries are independent
position/focus evidence, not an alternate input injector. Run only once the
root-owned X11 guard and package input service have been validated.
"""
import argparse
import ctypes
import ctypes.util
import json
import os
from pathlib import Path
import re
import select
import socket
import stat
import struct
import subprocess
import time

CLI = '/opt/donkeywork-desktop/current/bin/input-cli'
SOCKET = '/run/donkeywork-desktop-input/input.sock'
GUARD = Path('/run/donkeywork-desktop-guard/state.json')
AUTH = '/var/run/lightdm/root/:0'
PACKET = struct.Struct('@llHHi')


def run(command):
    return subprocess.run(command, check=True, stdin=subprocess.DEVNULL,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          timeout=5, text=True).stdout.strip()


def cli(*args):
    # Do not log CLI payloads. Only this script's fixed movement/modifier actions
    # are accepted below; the underlying helper still enforces its guard/lease.
    run([CLI, '--socket', SOCKET, *map(str, args)])


def check_guard():
    info = GUARD.stat(follow_symlinks=False)
    if not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022 or info.st_size > 4096:
        raise RuntimeError('trusted bounded guard required')
    if not 0 <= time.time() - info.st_mtime <= 0.5:
        raise RuntimeError('fresh guard required')
    data = json.loads(GUARD.read_text())
    if data.get('valid') is not True or (data.get('width'), data.get('height')) != (1920, 1080):
        raise RuntimeError('valid 1080p guard required')


def check_geometry():
    monitors = run(['/usr/bin/xrandr', '--listmonitors'])
    if not monitors.startswith('Monitors: 1\n'):
        raise RuntimeError('single XRandR monitor required')
    text = run(['/usr/bin/xrandr', '--verbose'])
    blocks = re.split(r'(?=^\S+ (?:connected|disconnected))', text, flags=re.M)
    active = [block for block in blocks if re.match(r'^\S+ connected(?: primary)? \d+x\d+[+-]\d+[+-]\d+', block)]
    if len(active) != 1 or not re.match(r'^HDMI-2 connected primary 1920x1080\+0\+0 \(0x[0-9a-f]+\) normal ', active[0]):
        raise RuntimeError('expected unchanged HDMI-2 single-output geometry')
    transform = re.search(r'Transform:\s*([^\n]+)\n\s*([^\n]+)\n\s*([^\n]+)', active[0])
    if not transform or [float(value) for line in transform.groups() for value in line.split()] != [1, 0, 0, 0, 1, 0, 0, 0, 1]:
        raise RuntimeError('identity XRandR transform required')
    connector = Path('/sys/class/drm/card1-HDMI-A-2')
    connector_id = int((connector / 'connector_id').read_text())
    advertised = re.search(r'CONNECTOR_ID:\s*(\d+)', active[0])
    if not advertised or int(advertised[1]) != connector_id:
        raise RuntimeError('XRandR output does not match captured DRM connector')
    if (connector / 'status').read_text().strip() != 'connected' or (connector / 'enabled').read_text().strip() != 'enabled':
        raise RuntimeError('physical HDMI connector is not active')
    drm = Path('/sys/kernel/debug/dri/1/state').read_text()
    crtc = re.search(r'^crtc\[88\]: pipe A\n(.*?)(?=^\w|\Z)', drm, re.M | re.S)
    physical = re.search(r'^connector\[' + str(connector_id) + r'\]: HDMI-A-2\n(.*?)(?=^\w|\Z)', drm, re.M | re.S)
    if not crtc or not physical or not re.search(r'^\s+active=1$', crtc[1], re.M) or not re.search(r'^\s+crtc=pipe A$', physical[1], re.M):
        raise RuntimeError('captured DRM CRTC/output binding changed')
    return {'xrandr': 'HDMI-2', 'drm': 'HDMI-A-2', 'connectorId': connector_id, 'crtc': 88, 'width': 1920, 'height': 1080}


class XQueries:
    def __init__(self):
        self.x = ctypes.CDLL(ctypes.util.find_library('X11'))
        self.x.XOpenDisplay.argtypes = [ctypes.c_char_p]
        self.x.XOpenDisplay.restype = ctypes.c_void_p
        self.x.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
        self.x.XDefaultRootWindow.restype = ctypes.c_ulong
        self.x.XQueryPointer.argtypes = [ctypes.c_void_p, ctypes.c_ulong,
            ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.c_ulong),
            ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int),
            ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int),
            ctypes.POINTER(ctypes.c_uint)]
        self.x.XQueryPointer.restype = ctypes.c_int
        self.x.XGetInputFocus.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.c_int)]
        self.x.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
        self.x.XKeysymToKeycode.restype = ctypes.c_ubyte
        self.x.XQueryKeymap.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        self.x.XCloseDisplay.argtypes = [ctypes.c_void_p]
        self.display = self.x.XOpenDisplay(b':0')
        if not self.display:
            raise RuntimeError('cannot query existing X11 console')
        self.root = self.x.XDefaultRootWindow(self.display)

    def pointer(self):
        root, child = ctypes.c_ulong(), ctypes.c_ulong()
        x, y, wx, wy = (ctypes.c_int() for _ in range(4))
        mask = ctypes.c_uint()
        if not self.x.XQueryPointer(self.display, self.root, ctypes.byref(root), ctypes.byref(child),
                ctypes.byref(x), ctypes.byref(y), ctypes.byref(wx), ctypes.byref(wy), ctypes.byref(mask)):
            raise RuntimeError('pointer is not on the captured X screen')
        return x.value, y.value, mask.value

    def focus(self):
        window, revert = ctypes.c_ulong(), ctypes.c_int()
        self.x.XGetInputFocus(self.display, ctypes.byref(window), ctypes.byref(revert))
        return window.value

    def shift_down(self):
        keycode = self.x.XKeysymToKeycode(self.display, 0xffe1)  # XK_Shift_L
        if not keycode:
            raise RuntimeError('X server has no left Shift mapping')
        bitmap = (ctypes.c_ubyte * 32)()
        self.x.XQueryKeymap(self.display, bitmap)
        return bool(bitmap[keycode // 8] & (1 << (keycode % 8)))

    def close(self):
        self.x.XCloseDisplay(self.display)


def owned_events():
    expected = {'DonkeyWork Console Keyboard', 'DonkeyWork Console Pointer'}
    paths = {}
    for candidate in Path('/sys/devices/virtual/input').glob('input[0-9]*'):
        try:
            name = (candidate / 'name').read_text().strip()
        except FileNotFoundError:
            continue
        if name not in expected:
            continue
        nodes = list(candidate.glob('event[0-9]*'))
        if name in paths or len(nodes) != 1:
            raise RuntimeError('ambiguous DonkeyWork input devices')
        paths[name] = Path('/dev/input') / nodes[0].name
    if set(paths) != expected:
        raise RuntimeError('exactly our virtual keyboard and pointer must exist')
    fds = {}
    try:
        for name, path in paths.items():
            descriptor = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC | os.O_NOFOLLOW)
            info = os.fstat(descriptor)
            if not stat.S_ISCHR(info.st_mode) or os.major(info.st_rdev) != 13:
                os.close(descriptor)
                raise RuntimeError('unexpected evdev character device')
            fds[descriptor] = name
        return fds
    except BaseException:
        for descriptor in fds:
            os.close(descriptor)
        raise


def drain(fds, duration=0.1):
    # Discard every unrelated event. Never print key contents or monitor a real
    # keyboard. We only retain the known test Shift and absolute X/Y samples.
    keys = set()
    axes = {}
    deadline = time.monotonic() + duration
    while time.monotonic() < deadline:
        ready, _, _ = select.select(list(fds), [], [], max(0, deadline - time.monotonic()))
        for descriptor in ready:
            data = os.read(descriptor, PACKET.size * 64)
            if not data or len(data) % PACKET.size:
                raise RuntimeError('virtual event device closed or malformed')
            for offset in range(0, len(data), PACKET.size):
                _, _, kind, code, value = PACKET.unpack_from(data, offset)
                if kind == 1 and value == 1 and (fds[descriptor] == 'DonkeyWork Console Pointer' or code != 42):
                    raise RuntimeError('unexpected button/non-Shift key down on owned test device')
                if fds[descriptor] == 'DonkeyWork Console Keyboard' and kind == 1 and code == 42 and value in (0, 1):
                    keys.add(value)
                if fds[descriptor] == 'DonkeyWork Console Pointer' and kind == 3 and code in (0, 1):
                    axes[code] = value
    return keys, axes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--allow-modifier-pointer-probe', action='store_true')
    parser.add_argument('--browser', action='store_true', help='Use actual browser input instead of CLI for actions')
    args = parser.parse_args()
    if not args.allow_modifier_pointer_probe or os.geteuid() != 0 or socket.gethostname() != 'easternkingdoms':
        parser.error('explicit flag, root, and easternkingdoms host required')
    for path in [Path(AUTH), Path(CLI)]:
        info = path.stat(follow_symlinks=False)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
            raise RuntimeError('root-owned non-writable executable/authority required')
    os.environ['DISPLAY'] = ':0'
    os.environ['XAUTHORITY'] = AUTH
    session = run(['loginctl', 'show-seat', 'seat0', '-p', 'ActiveSession', '--value'])
    if run(['loginctl', 'show-session', session, '-p', 'Type', '--value']) != 'x11':
        raise RuntimeError('existing X11 seat required')
    geometry = check_geometry()
    check_guard()
    x = XQueries()
    fds = {}
    moved = False
    initial = x.pointer()
    focus = x.focus()
    # Ignore lock-state bits; reject held modifiers or buttons from a person.
    if initial[2] & (1 | 4 | 8 | 64 | 256 | 512 | 1024 | 2048 | 4096):
        x.close()
        raise RuntimeError('operator is holding input; refusing to test')
    results = []
    try:
        fds = owned_events()
        cli('reset')  # Fails before motion if another controller owns the helper.
        drain(fds)
        if args.browser:
            moved = True
            browser = subprocess.Popen(['/usr/bin/node', str(Path(__file__).with_name('easternkingdoms-browser-input.mjs')),
                                        '--allow-modifier-pointer-probe'], stdin=subprocess.PIPE,
                                       stdout=subprocess.PIPE, stderr=None, text=True)
            try:
                seen = []
                shift_events = set()
                for _ in range(7):
                    if not select.select([browser.stdout], [], [], 40)[0]:
                        raise RuntimeError('browser evidence timeout')
                    event = json.loads(browser.stdout.readline())
                    check_guard()
                    keys, axes = drain(fds, 0.2)
                    shift_events.update(keys)
                    if event['type'] == 'move':
                        actual = x.pointer()
                        if any(abs(actual[i] - event[k]) > 1 for i, k in enumerate(('x', 'y'))):
                            raise RuntimeError('browser coordinates differ from independently observed X11 pointer')
                        if any(abs(axes.get(i, -10000) - event[k]) > 1 for i, k in enumerate(('x', 'y'))):
                            raise RuntimeError('browser motion missing from owned evdev pointer')
                    if event['type'] == 'shiftDown' and not x.shift_down():
                        raise RuntimeError('browser Shift not held in X server keymap')
                    if event['type'] == 'shift' and (shift_events != {0, 1} or x.shift_down()):
                        raise RuntimeError('browser Shift down/up missing from owned evdev keyboard')
                    if x.focus() != focus:
                        raise RuntimeError('browser changed desktop window focus')
                    seen.append(event)
                    browser.stdin.write('ok\n')
                    browser.stdin.flush()
                if browser.wait(timeout=10) != 0 or [v['type'] for v in seen] != ['acquired','move','move','move','shiftDown','shift','released']:
                    raise RuntimeError('browser pipeline incomplete')
                print(json.dumps({'host':'easternkingdoms','browserPipeline':seen,'independentEvdevAndXQueryPointer':True,'xServerShiftDownUpObserved':True}))
            finally:
                if browser.poll() is None:
                    browser.terminate()
                    browser.wait(timeout=5)
            return
        for px, py in [(200, 200), (960, 540), (1720, 880)]:
            check_guard()
            moved = True
            cli('move', px, py)
            _, axes = drain(fds)
            deadline = time.monotonic() + 1
            actual = x.pointer()
            while time.monotonic() < deadline and (abs(actual[0] - px) > 1 or abs(actual[1] - py) > 1):
                time.sleep(0.02)
                actual = x.pointer()
            if axes.get(0) != px or axes.get(1) != py:
                raise RuntimeError('owned virtual pointer did not emit the requested coordinates')
            if abs(actual[0] - px) > 1 or abs(actual[1] - py) > 1:
                raise RuntimeError(f'X11 pointer mismatch for known probe requested={(px, py)} observed={actual[:2]}')
            if x.focus() != focus:
                raise RuntimeError('pointer movement changed window focus; stopping')
            results.append({'requested': [px, py], 'observed': list(actual[:2]), 'ownedDeviceObserved': True})
        check_guard()
        drain(fds)
        key = subprocess.Popen([CLI, '--socket', SOCKET, 'key', '225', '--hold-ms', '250'],
                               stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        held = False
        deadline = time.monotonic() + 0.5
        try:
            while time.monotonic() < deadline and key.poll() is None:
                held = x.shift_down() or held
                time.sleep(0.01)
            if key.wait(timeout=1) != 0:
                raise RuntimeError('bounded standalone Shift CLI failed')
        finally:
            if key.poll() is None:
                key.terminate()
                key.wait(timeout=1)
        keys, _ = drain(fds, 0.2)
        if keys != {0, 1} or not held or x.shift_down():
            raise RuntimeError('owned keyboard Shift down/up was not independently observed')
        if x.focus() != focus:
            raise RuntimeError('original window focus changed')
        print(json.dumps({'host': 'easternkingdoms', 'geometry': geometry, 'pointer': results,
                          'standaloneShiftDownUpObserved': True, 'xServerShiftDownUpObserved': True, 'windowFocusUnchanged': True}))
    finally:
        try:
            if moved:
                cli('move', initial[0], initial[1])
                deadline = time.monotonic() + 1
                while time.monotonic() < deadline and x.pointer()[:2] != initial[:2]:
                    time.sleep(0.02)
                if any(abs(a - b) > 1 for a, b in zip(x.pointer()[:2], initial[:2])):
                    raise RuntimeError('original pointer restoration failed; no X11 injection fallback attempted')
                if x.focus() != focus:
                    raise RuntimeError('original focus was not preserved; no focus-stealing fallback attempted')
                print(json.dumps({'originalPointerRestored': True, 'originalFocusPreserved': True}))
        finally:
            for descriptor in fds:
                os.close(descriptor)
            x.close()


if __name__ == '__main__':
    main()
