"""Read-only X11 target discovery and conservative single-output validation."""
import json
import os
from pathlib import Path
import re
import selectors
import stat
import subprocess
import time

DISPLAY = re.compile(r'^:[0-9]+(?:\.0)?$')
IDENTITY = (1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0)


def active_session():
    import dbus
    bus = dbus.SystemBus()
    try:
        properties = dbus.Interface(bus.get_object('org.freedesktop.login1',
            '/org/freedesktop/login1/seat/seat0'), 'org.freedesktop.DBus.Properties')
        active = properties.Get('org.freedesktop.login1.Seat', 'ActiveSession', timeout=0.2)
        if not str(active[0]) or str(active[1]) == '/':
            raise ValueError('no active graphical session')
        properties = dbus.Interface(bus.get_object('org.freedesktop.login1', active[1]),
                                    'org.freedesktop.DBus.Properties')
        values = properties.GetAll('org.freedesktop.login1.Session', timeout=0.2)
        if str(values['Type']) != 'x11' or str(values['Class']) not in ('user', 'greeter') or not bool(values['Active']):
            raise ValueError('active seat is not an X11 greeter/user')
        return {'session': str(active[0]), 'uid': int(values['User'][0]),
                'display': str(values.get('Display', '')), 'vt': int(values.get('VTNr', 0)),
                'locked': bool(values.get('LockedHint', False))}
    finally:
        bus.close()


def authority_identity(path, uid):
    path = Path(path)
    if not path.is_absolute() or '..' in path.parts:
        raise ValueError('authority must be an absolute path without traversal')
    info = path.stat(follow_symlinks=False)
    if not stat.S_ISREG(info.st_mode) or info.st_uid not in (0, uid) or info.st_mode & 0o077:
        raise ValueError('authority must be a private regular file owned by root/active account')
    # Metadata only: never read or report the authentication cookie.
    return (str(path.resolve(strict=True)), info.st_dev, info.st_ino, info.st_mtime_ns, info.st_size)


def capture_identity(pid, display, authority, width, height, drm_state=None, proc=Path('/proc')):
    directory = proc / str(pid)
    if directory.stat().st_uid != 0:
        raise ValueError('capture process must be root-owned')
    raw = (directory / 'cmdline').read_bytes()
    if not raw or len(raw) > 16384:
        raise ValueError('capture command line missing or oversized')
    argv = [value.decode() for value in raw.rstrip(b'\0').split(b'\0')]
    if Path(argv[0]).name != 'dwconsole-daemon':
        raise ValueError('capture pid is not the console daemon')
    def option(name):
        if argv.count(name) != 1:
            raise ValueError('capture option missing or duplicated')
        position = argv.index(name) + 1
        if position >= len(argv):
            raise ValueError('capture option value missing')
        return argv[position]
    if '--x11-display' in argv:
        if (option('--x11-display') != display
                or os.path.realpath(option('--xauthority')) != os.path.realpath(authority)
                or (int(option('--width')), int(option('--height'))) != (width, height)):
            raise ValueError('X11 input target differs from captured source')
    else:
        if drm_state is None:
            raise ValueError('KMS capture requires matching DRM-state validation')
        device = os.stat(option('--device'), follow_symlinks=False)
        expected = Path('/sys/kernel/debug/dri') / str(os.minor(device.st_rdev)) / 'state'
        if not stat.S_ISCHR(device.st_mode) or os.major(device.st_rdev) != 226 or Path(drm_state) != expected:
            raise ValueError('DRM state does not belong to the capture device')
    process = (directory / 'stat').read_text()
    fields = process[process.rfind(')') + 2:].split()
    if len(fields) < 20 or fields[0] == 'Z':
        raise ValueError('capture process unavailable')
    return (pid, fields[19], tuple(argv))


def bounded_command(command, environment, timeout=0.25, limit=131072):
    process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.DEVNULL, env=environment)
    output = bytearray()
    try:
        deadline = time.monotonic() + timeout
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise TimeoutError('X11 query exceeded deadline')
                if not selector.select(remaining):
                    raise TimeoutError('X11 query exceeded deadline')
                chunk = os.read(process.stdout.fileno(), min(4096, limit + 1 - len(output)))
                if not chunk:
                    if process.wait(timeout=max(0.001, deadline - time.monotonic())):
                        raise ValueError('X11 query failed')
                    return output.decode('utf-8', errors='strict')
                output.extend(chunk)
                if len(output) > limit:
                    raise ValueError('X11 query exceeded output bound')
    finally:
        if process.poll() is None:
            process.kill()
        process.wait()
        process.stdout.close()


def parse_randr(text, width, height, require_connector=False):
    screens = re.findall(r'^Screen (\d+):.*?current (\d+) x (\d+),', text, re.M)
    if screens != [('0', str(width), str(height))]:
        raise ValueError('X11 root dimensions differ from the captured source')
    headers = list(re.finditer(r'^(\S+) (?:connected|disconnected)\b[^\n]*', text, re.M))
    active = []
    for index, header in enumerate(headers):
        line = header.group(0)
        if ' connected' not in line or not re.search(r'\d+x\d+[+-]\d+[+-]\d+', line):
            continue
        geometry = re.fullmatch(r'(\S+) connected(?: primary)? (\d+)x(\d+)([+-]\d+)([+-]\d+) \((0x[0-9a-fA-F]+)\) normal \([^\n]*', line)
        if not geometry or tuple(map(int, geometry.groups()[1:5])) != (width, height, 0, 0):
            raise ValueError('X11 output is rotated, offset, or not the full source raster')
        block = text[header.end():headers[index + 1].start() if index + 1 < len(headers) else len(text)]
        matrix = re.search(r'^\s+Transform:\s+([^\n]+)\n\s+([^\n]+)\n\s+([^\n]+)', block, re.M)
        if not matrix or tuple(float(value) for row in matrix.groups() for value in row.split()) != IDENTITY:
            raise ValueError('X11 output transform is not identity')
        panning = re.findall(r'^\s+[Pp]anning:\s*(.*)$', block, re.M)
        if any(value.strip() != '0x0+0+0' for value in panning):
            raise ValueError('X11 panning is unsupported')
        timestamp = re.findall(r'^\s+Timestamp:\s+(-?\d+)\s*$', block, re.M)
        crtc = re.findall(r'^\s+CRTC:\s+(\d+)\s*$', block, re.M)
        modes = re.findall(r'^\s+\S+ \((0x[0-9a-fA-F]+)\)[^\n]*\*current\b', block, re.M)
        if len(timestamp) != 1 or len(crtc) != 1 or modes != [geometry.group(6)]:
            raise ValueError('X11 active mode identity is ambiguous')
        identity = (geometry.group(1), geometry.group(6), timestamp[0], crtc[0])
        if require_connector:
            connectors = re.findall(r'^\s+CONNECTOR_ID:\s+(\d+)\s*$', block, re.M)
            if len(connectors) != 1 or int(connectors[0]) <= 0:
                raise ValueError('X11 DRM connector identity missing or ambiguous')
            identity += (connectors[0],)
        active.append(identity)
    if len(active) != 1:
        raise ValueError('exactly one active X11 output is required')
    return active[0]


def randr_snapshot(display, authority, width, height, require_connector=False):
    if not DISPLAY.fullmatch(display):
        raise ValueError('only local X11 screen zero is supported')
    # Root is intentional: LightDM's root-only authority is not readable by the user.
    environment = {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C', 'DISPLAY': display, 'XAUTHORITY': authority}
    text = bounded_command(['/usr/bin/xrandr', '--verbose', '--current'], environment)
    return parse_randr(text, width, height, require_connector)


def drm_snapshot(path, width, height, connector_id=None):
    with open(path) as source:
        text = source.read(65537)
    if len(text) > 65536:
        raise ValueError('DRM state exceeds bound')
    blocks = re.findall(r'^crtc\[\d+\]:.*?(?=^\w|\Z)', text, re.M | re.S)
    active = [block for block in blocks if re.search(r'^\s+active=1$', block, re.M)]
    if len(active) != 1:
        raise ValueError('KMS output does not match X11 raster')
    modes = re.findall(r'^\s+mode: "[^"\n]*": (\d+) (\d+) (\d+) (\d+) (\d+) (\d+) (\d+) (\d+) (\d+) (\d+) (0x[0-9a-fA-F]+) (0x[0-9a-fA-F]+)\s*$', active[0], re.M)
    if len(modes) != 1:
        raise ValueError('KMS active timing is missing or ambiguous')
    refresh, clock, horizontal, hstart, hend, htotal, vertical, vstart, vend, vtotal = map(int, modes[0][:10])
    if (horizontal, vertical) != (width, height) or not (refresh > 0 and clock > 0
            and horizontal <= hstart <= hend <= htotal
            and vertical <= vstart <= vend <= vtotal):
        raise ValueError('KMS output does not match X11 raster')
    identity = (active[0].splitlines()[0], next(line.strip() for line in active[0].splitlines() if line.strip().startswith('mode:')))
    if connector_id is not None:
        crtc = re.fullmatch(r'crtc\[(\d+)\]:\s*(\S.*)', identity[0])
        connectors = re.findall(r'^connector\[\d+\]:.*?(?=^\w|\Z)', text, re.M | re.S)
        matching = [block for block in connectors if block.startswith('connector[' + str(connector_id) + ']:')]
        if not crtc or len(matching) != 1:
            raise ValueError('captured DRM connector differs from X11 output')
        linked = re.findall(r'^\s+crtc=(.*)$', matching[0], re.M)
        if linked != [crtc.group(2)]:
            raise ValueError('X11 connector is not attached to captured CRTC')
        identity += (matching[0].splitlines()[0], linked[0])
    return identity


def discover_target(capture_pid=None, width=None, height=None, drm_state=None, proc=Path('/proc')):
    session = active_session()
    expected_dimensions = None if width is None and height is None else (width, height)
    uid = session['uid']
    candidates = set()
    xorg_candidates = set()
    shells = {'gnome-shell', 'xfce4-session', 'plasmashell', 'cinnamon', 'mate-session', 'openbox'}
    for directory in proc.iterdir():
        if not directory.name.isdigit():
            continue
        try:
            comm = (directory / 'comm').read_text().strip()
            if comm in shells and directory.stat().st_uid == uid:
                raw = (directory / 'environ').read_bytes()
                if len(raw) > 1024 * 1024:
                    continue
                values = {}
                for entry in raw.split(b'\0'):
                    key, _, value = entry.partition(b'=')
                    if key in (b'DISPLAY', b'XAUTHORITY', b'XDG_SESSION_ID'):
                        values[key.decode()] = value.decode()
                if values.get('XDG_SESSION_ID', session['session']) != session['session']:
                    continue
                if 'DISPLAY' in values and 'XAUTHORITY' in values:
                    candidates.add((values['DISPLAY'], values['XAUTHORITY']))
            elif comm == 'Xorg':
                raw = (directory / 'cmdline').read_bytes()
                if len(raw) > 16384:
                    continue
                argv = [part.decode() for part in raw.rstrip(b'\0').split(b'\0')]
                displays = [part for part in argv if DISPLAY.fullmatch(part)]
                if session['vt'] and 'vt' + str(session['vt']) in argv and len(displays) == 1 and argv.count('-auth') == 1:
                    candidate = (displays[0], argv[argv.index('-auth') + 1])
                    candidates.add(candidate)
                    if directory.stat().st_uid == 0:
                        xorg_candidates.add(candidate)
        except (FileNotFoundError, ProcessLookupError, PermissionError, IndexError, UnicodeError):
            continue
    verified = {}
    for display, authority in candidates:
        if not DISPLAY.fullmatch(display) or (session['display'] and display != session['display']):
            continue
        try:
            identity = authority_identity(authority, uid)
            # Query root geometry before applying the strict mapping check.
            text = bounded_command(['/usr/bin/xrandr', '--verbose', '--current'],
                {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C', 'DISPLAY': display, 'XAUTHORITY': authority})
            match = re.search(r'^Screen 0:.*?current (\d+) x (\d+),', text, re.M)
            if not match:
                continue
            width, height = map(int, match.groups())
            if not 2 <= width <= 4096 or not 2 <= height <= 4096:
                continue
            if expected_dimensions is not None and (width, height) != expected_dimensions:
                continue
            layout = parse_randr(text, width, height, drm_state is not None)
            if capture_pid is not None:
                capture_identity(capture_pid, display, authority, width, height, drm_state, proc)
            if drm_state is not None:
                drm_snapshot(drm_state, width, height, layout[4])
            verified[(display, identity[0])] = (width, height)
        except (OSError, ValueError, TimeoutError):
            continue
    # A desktop may copy the server cookie into its own private authority file.
    # Only collapse aliases for the same display, using its root Xorg/VT authority.
    if len(verified) > 1 and len({display for display, _ in verified}) == 1:
        preferred = {key: dimensions for key, dimensions in verified.items()
                     if any(key == (display, os.path.realpath(authority))
                            for display, authority in xorg_candidates)}
        if len(preferred) == 1:
            verified = preferred
    if len(verified) != 1:
        raise ValueError('active X11 display/authority is missing or ambiguous')
    (display, authority), (width, height) = next(iter(verified.items()))
    return session | {'display': display, 'xauthority': authority, 'width': width, 'height': height}


if __name__ == '__main__':
    print(json.dumps(discover_target()))
