"""Package GNOME/Wayland seat supervisor (adapted from our tested pilot). Never restarts GDM or user applications.

Session changes own only output/guard units; capture, input and web stay alive.
SIGTERM stops those session units and invalidates input. It does not log out the session,
restore display layout, unload VKMS, or stop capture. Boot installation is separate.
"""
import argparse
import ipaddress
import json
import os
from pathlib import Path
import pwd
import re
import signal
import stat
import subprocess
import threading
import time
from urllib.parse import unquote

UNITS = ('donkeywork-desktop-input-guard', 'donkeywork-desktop-vkms-output')


def local_bus(address):
    """Accept exactly one local Unix transport, including private GDM buses."""
    if not address.startswith('unix:') or ';' in address or '\0' in address:
        raise ValueError('session bus must use one local Unix transport')
    fields = {}
    for item in address[5:].split(','):
        key, value = item.split('=', 1)
        if key in fields or key not in ('path', 'abstract', 'guid'):
            raise ValueError('invalid Unix bus option')
        fields[key] = unquote(value)
    transports = [key for key in ('path', 'abstract') if key in fields]
    if len(transports) != 1:
        raise ValueError('ambiguous Unix bus address')
    value = fields[transports[0]]
    if not value or '\0' in value or (transports[0] == 'path' and not value.startswith('/')):
        raise ValueError('invalid Unix bus endpoint')
    if 'guid' in fields and not re.fullmatch('[0-9a-fA-F]{32}', fields['guid']):
        raise ValueError('invalid bus GUID')
    return address


def discover_shell(uid, proc=Path('/proc')):
    matches = []
    for directory in proc.iterdir():
        if not directory.name.isdigit():
            continue
        try:
            if directory.stat().st_uid != uid or (directory / 'comm').read_text().strip() != 'gnome-shell':
                continue
            raw = (directory / 'environ').read_bytes()
            if len(raw) > 1024 * 1024:
                raise ValueError('shell environment exceeds limit')
            values = [entry.split(b'=', 1)[1] for entry in raw.split(b'\0')
                      if entry.startswith(b'DBUS_SESSION_BUS_ADDRESS=')]
            if len(values) != 1:
                raise ValueError('shell bus is missing or ambiguous')
            matches.append((int(directory.name), local_bus(values[0].decode())))
        except (FileNotFoundError, ProcessLookupError):
            continue
    if len(matches) != 1:
        raise ValueError('expected exactly one GNOME Shell for active account')
    return matches[0]


def topology_ready(state):
    _, monitors, logical, _ = state
    if len(logical) != 1:
        return False
    x, y, scale, transform, _, outputs, _ = logical[0]
    if (int(x), int(y), float(scale), int(transform)) != (0, 0, 1.0, 0) or len(outputs) != 1:
        return False
    connector = str(outputs[0][0])
    if connector != 'Virtual-1':
        return False
    current = [mode for spec, modes, _ in monitors if str(spec[0]) == connector
               for mode in modes if mode[6].get('is-current', False)]
    return len(current) == 1 and (int(current[0][1]), int(current[0][2])) == (1920, 1080)


def drm_ready(path):
    # Mutter may publish its requested mode before the kernel commit completes.
    # Wait for both before starting the guard; never assume a startup delay.
    try:
        with open(path) as source:
            text = source.read(65536)
        blocks = re.findall(r'^crtc\[\d+\]:.*?(?=^\w|\Z)', text, re.M | re.S)
        active = [block for block in blocks if re.search(r'^\s+active=1$', block, re.M)]
        return len(active) == 1 and 'mode: "1920x1080"' in active[0]
    except OSError:
        return False


def valid_guard(path, session, not_before=0):
    try:
        info = os.stat(path, follow_symlinks=False)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
            return False
        if info.st_size > 4096 or info.st_mtime < not_before or not 0 <= time.time() - info.st_mtime <= 0.5:
            return False
        with open(path) as source:
            value = json.load(source)
        return (value.get('valid') is True and value.get('width') == 1920
                and value.get('height') == 1080
                and str(value.get('session', '')).startswith(session + ':'))
    except (OSError, ValueError):
        return False


class Supervisor:
    def __init__(self, args):
        import dbus
        self.dbus = dbus
        self.args = args
        self.stop = threading.Event()
        self.system = dbus.SystemBus()
        self.seat = dbus.Interface(self.system.get_object('org.freedesktop.login1',
            '/org/freedesktop/login1/seat/seat0'), 'org.freedesktop.DBus.Properties')

    def target(self):
        active = self.seat.Get('org.freedesktop.login1.Seat', 'ActiveSession', timeout=0.2)
        if not str(active[0]) or str(active[1]) == '/':
            raise ValueError('no active graphical seat session')
        props = self.dbus.Interface(self.system.get_object('org.freedesktop.login1', active[1]),
                                   'org.freedesktop.DBus.Properties')
        session = props.GetAll('org.freedesktop.login1.Session', timeout=0.2)
        if (str(session['Type']) != 'wayland' or str(session['Class']) not in ('greeter', 'user')
                or not bool(session['Active'])):
            raise ValueError('active session is not supported GNOME Wayland')
        uid = int(session['User'][0])
        pid, address = discover_shell(uid)
        return str(active[0]), uid, pid, address, str(session['Class'])

    def command(self, command, check=True):
        result = subprocess.run(command, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                stderr=subprocess.DEVNULL, timeout=8)
        if check and result.returncode:
            raise RuntimeError('task unit operation failed')

    def stop_units(self):
        # Invalidating the guard releases held input without disconnecting video.
        for unit in UNITS:
            self.command(['systemctl', 'stop', unit + '.service'], check=False)

    def start_unit(self, name, command, properties=(), environment=()):
        self.command(['systemctl', 'reset-failed', name + '.service'], check=False)
        self.command(['systemd-run', '--quiet', '--collect', '--unit=' + name,
                         '--property=PartOf=donkeywork-desktop.target', '--property=NoNewPrivileges=yes']
                     + ['--property=' + value for value in properties]
                     + ['--setenv=' + value for value in environment] + ['--'] + command)

    def active(self, unit):
        return subprocess.run(['systemctl', 'is-active', '--quiet', unit],
                              stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL, timeout=2).returncode == 0

    def wait_ready(self, target, condition, duration=5):
        deadline = time.monotonic() + duration
        while not self.stop.is_set() and time.monotonic() < deadline:
            if self.target() != target:
                raise RuntimeError('seat changed during preparation')
            if condition():
                return
            # Condition polling, bounded by deadline; never an assumed startup sleep.
            self.stop.wait(0.1)
        raise RuntimeError('console preparation timed out or stopped')

    def start_web(self, control):
        self.command(['systemctl', 'start', 'donkeywork-desktop-web.service'])

    def prepare(self, target):
        session, uid, _, address, session_class = target
        args = self.args
        self.stop_units()
        # Authenticate as the actual session account, then restore root publication authority.
        os.seteuid(uid)
        try:
            private = self.dbus.bus.BusConnection(address)
        finally:
            os.seteuid(0)
        try:
            display = self.dbus.Interface(private.get_object('org.gnome.Mutter.DisplayConfig',
                '/org/gnome/Mutter/DisplayConfig'), 'org.gnome.Mutter.DisplayConfig')
            if session_class == 'greeter':
                self.start_unit('donkeywork-desktop-vkms-output', ['/usr/bin/python3',
                                str(args.libexec_dir / 'keep-console-output.py')],
                                ['User=' + str(uid), 'Group=' + str(pwd.getpwuid(uid).pw_gid)],
                                ['DBUS_SESSION_BUS_ADDRESS=' + address, 'XDG_RUNTIME_DIR=/run/user/' + str(uid)])
                self.wait_ready(target, lambda: topology_ready(display.GetCurrentState(timeout=0.2))
                                and drm_ready(args.drm_state))
        finally:
            private.close()
        guard_started = time.time()
        self.start_unit('donkeywork-desktop-input-guard', ['/usr/bin/python3',
            str(args.libexec_dir / 'input-target-guard.py'), '--session', session,
            '--session-uid', str(uid), '--session-bus', address, '--drm-state', str(args.drm_state),
            '--output', args.guard_state], ['RuntimeDirectory=donkeywork-desktop-guard', 'RuntimeDirectoryMode=0700'])
        self.wait_ready(target, lambda: valid_guard(args.guard_state, session, guard_started))
        if not self.active('donkeywork-desktop-input'):
            self.command(['systemctl', 'start', 'donkeywork-desktop-input.service'])
        def socket_ready():
            try:
                info = os.stat(args.input_socket, follow_symlinks=False)
                return stat.S_ISSOCK(info.st_mode) and info.st_uid == 0 and not info.st_mode & 0o077
            except FileNotFoundError:
                return False
        self.wait_ready(target, socket_ready)
        if not self.active('donkeywork-desktop-broker'):
            self.command(['systemctl', 'start', 'donkeywork-desktop-broker.service'])
        self.wait_ready(target, lambda: Path(args.broker_socket).is_socket())
        self.start_web(True)

    def run(self):
        current = None
        retry_at = 0
        last_notice = None
        try:
            while not self.stop.is_set():
                if time.monotonic() < retry_at:
                    self.stop.wait(0.25)
                    continue
                try:
                    target = self.target()
                    if target != current or not valid_guard(self.args.guard_state, target[0]):
                        if time.monotonic() >= retry_at:
                            self.prepare(target)
                            current = target
                            retry_at = 0
                            print('active Wayland console output and control ready', flush=True)
                            last_notice = None
                except Exception as error:
                    self.stop_units()
                    current = None
                    # Existing viewer remains connected while capture recovers.
                    # Never replace its process with a display-only instance.
                    notice = type(error).__name__
                    if notice != last_notice:
                        print('console waiting for supported active output; input disabled (' + notice + ')', flush=True)
                        last_notice = notice
                    retry_at = time.monotonic() + 2
                self.stop.wait(0.25)
        finally:
            self.stop_units()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--host-ip', default='192.168.69.21')
    parser.add_argument('--bin-dir', type=Path, default=Path('/opt/donkeywork-desktop/current/bin'))
    parser.add_argument('--libexec-dir', type=Path, default=Path('/opt/donkeywork-desktop/current/libexec'))
    parser.add_argument('--assets', type=Path, default=Path('/opt/donkeywork-desktop/current/share/web'))
    parser.add_argument('--drm-state', type=Path, default=Path('/sys/kernel/debug/dri/2/state'))
    parser.add_argument('--capture-socket', default='/run/donkeywork-desktop-capture/stream.sock')
    parser.add_argument('--input-socket', default='/run/donkeywork-desktop-input/input.sock')
    parser.add_argument('--guard-state', default='/run/donkeywork-desktop-guard/state.json')
    parser.add_argument('--broker-socket', default='/run/donkeywork-desktop-broker/broker.sock')
    args = parser.parse_args()
    address = ipaddress.ip_address(args.host_ip)
    if os.geteuid() != 0 or address.version != 4 or not address.is_private:
        parser.error('root and a private IPv4 host address are required')
    for path in (args.bin_dir, args.libexec_dir, args.assets, args.drm_state, Path(args.capture_socket),
                 Path(args.input_socket), Path(args.guard_state), Path(args.broker_socket)):
        if not path.is_absolute():
            parser.error('all paths must be absolute')
    # Unit RuntimeDirectory names are deliberately fixed for this pilot.
    if (Path(args.input_socket).parent != Path('/run/donkeywork-desktop-input')
            or Path(args.guard_state).parent != Path('/run/donkeywork-desktop-guard')
            or Path(args.broker_socket).parent != Path('/run/donkeywork-desktop-broker')):
        parser.error('input and guard paths must use their dedicated runtime directories')
    for path in [args.libexec_dir / 'keep-console-output.py', args.libexec_dir / 'input-target-guard.py',
                 args.bin_dir / 'input-daemon', args.bin_dir / 'web-launch',
                 args.bin_dir / 'console-web', args.bin_dir / 'socket-broker']:
        info = path.stat(follow_symlinks=False)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
            parser.error('package executable/script must be a root-owned non-writable regular file')
    supervisor = Supervisor(args)
    signal.signal(signal.SIGTERM, lambda *_: supervisor.stop.set())
    signal.signal(signal.SIGINT, lambda *_: supervisor.stop.set())
    supervisor.run()


if __name__ == '__main__':
    main()
