#!/usr/bin/python3
"""Package unit entrypoints. Configuration is parsed data, never shell code."""
import argparse
import ipaddress
import os
from pathlib import Path
import re
import stat
import sys
import time

CONFIG = Path('/etc/donkeywork-desktop/console.env')
CURRENT = Path('/opt/donkeywork-desktop/current')
CAPTURE = '/run/donkeywork-desktop-capture/stream.sock'
BROKER = '/run/donkeywork-desktop-broker/broker.sock'
INPUT = '/run/donkeywork-desktop-input/input.sock'
GUARD = '/run/donkeywork-desktop-guard/state.json'
DEFAULTS = {'DRM_DEVICE': 'auto', 'WIDTH': '1920', 'HEIGHT': '1080', 'CONTROL': 'auto',
            'FFMPEG': '/usr/bin/ffmpeg',
            'BIN_DIR': str(CURRENT / 'bin'), 'ASSETS': str(CURRENT / 'share/web'),
            'LIBEXEC_DIR': str(CURRENT / 'libexec')}
KEYS = set(DEFAULTS) | {'PROFILE', 'LISTEN_IP'}


def parse_config(text):
    if len(text.encode()) > 8192:
        raise ValueError('configuration exceeds 8192 bytes')
    values = {}
    for line in text.splitlines():
        if not line or line.startswith('#'):
            continue
        if '=' not in line:
            raise ValueError('configuration requires KEY=value lines')
        key, value = line.split('=', 1)
        if key not in KEYS or key in values or not value or not re.fullmatch(r'[A-Za-z0-9_./:+-]+', value):
            raise ValueError('unknown, duplicate or invalid configuration value')
        values[key] = value
    values = DEFAULTS | values
    if values.get('PROFILE') not in ('vkms', 'kms', 'x11'):
        raise ValueError('PROFILE must be vkms, kms or x11')
    if values['CONTROL'] not in ('auto', 'x11') or (values['CONTROL'] == 'x11' and values['PROFILE'] == 'vkms'):
        raise ValueError('CONTROL=x11 requires an X11 or physical KMS capture profile')
    address = ipaddress.ip_address(values.get('LISTEN_IP', ''))
    networks = ('10.0.0.0/8', '172.16.0.0/12', '192.168.0.0/16')
    if address.version != 4 or not any(address in ipaddress.ip_network(net) for net in networks):
        raise ValueError('LISTEN_IP must be an explicit RFC1918 IPv4 address')
    for key in ('WIDTH', 'HEIGHT'):
        if not values[key].isdigit() or not 2 <= int(values[key]) <= 4096:
            raise ValueError('invalid configured dimensions')
    if values['PROFILE'] == 'vkms' and (values['WIDTH'], values['HEIGHT']) != ('1920', '1080'):
        raise ValueError('VKMS pilot supports only 1920x1080')
    if values['DRM_DEVICE'] != 'auto' and not re.fullmatch(r'/dev/dri/card[0-9]+', values['DRM_DEVICE']):
        raise ValueError('DRM_DEVICE must be auto or an explicit DRM card')
    if values['PROFILE'] == 'kms' and values['DRM_DEVICE'] == 'auto':
        raise ValueError('KMS view-only profile requires an explicit DRM_DEVICE')
    if values['PROFILE'] == 'x11' and values['DRM_DEVICE'] == 'auto':
        values['DRM_DEVICE'] = '/dev/dri/card0'
    ffmpeg = Path(values['FFMPEG'])
    if not ffmpeg.is_absolute() or '..' in ffmpeg.parts:
        raise ValueError('FFMPEG must be an absolute executable path without parent traversal')
    for key in ('BIN_DIR', 'ASSETS', 'LIBEXEC_DIR'):
        if values[key] != DEFAULTS[key]:
            raise ValueError('package paths must use the fixed current release layout')
    return values


def load_config(path=CONFIG):
    parent = path.parent.stat(follow_symlinks=False)
    if not stat.S_ISDIR(parent.st_mode) or parent.st_uid != 0 or parent.st_mode & 0o022:
        raise ValueError('configuration directory must be root-owned and not writable by others')
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
    with os.fdopen(descriptor) as source:
        info = os.fstat(source.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022 or info.st_size > 8192:
            raise ValueError('configuration must be a bounded root-owned non-writable regular file')
        return parse_config(source.read(8193))


def choose_drm(config, sysfs=Path('/sys/class/drm'), devices=Path('/dev/dri')):
    candidates = []
    for card in sysfs.glob('card[0-9]*'):
        if not re.fullmatch(r'card[0-9]+', card.name):
            continue
        try:
            device = card / 'device'
            driver = device / 'driver'
            # Linux 6.8 VKMS registers a platform device without a driver link.
            platform_vkms = ((device / 'subsystem').resolve().name == 'platform'
                             and 'MODALIAS=platform:vkms' in (device / 'uevent').read_text().splitlines()) if not driver.exists() else False
            if (driver.exists() and driver.resolve(strict=True).name == 'vkms') or platform_vkms:
                candidates.append(devices / card.name)
        except FileNotFoundError:
            continue
    if config['DRM_DEVICE'] == 'auto':
        if config['PROFILE'] != 'vkms' or len(candidates) != 1:
            raise ValueError('exactly one loaded VKMS card is required; installer does not load it implicitly')
        selected = candidates[0]
    else:
        selected = Path(config['DRM_DEVICE'])
    if config['PROFILE'] == 'vkms' and selected not in candidates:
        raise ValueError('VKMS profile refuses a physical GPU card')
    return selected


def drm_paths(config):
    device = choose_drm(config)
    info = device.stat(follow_symlinks=False)
    if not stat.S_ISCHR(info.st_mode) or os.major(info.st_rdev) != 226:
        raise ValueError('configured DRM card is not a DRM character device')
    return str(device), '/sys/kernel/debug/dri/' + str(os.minor(info.st_rdev)) + '/state'


def trusted_file(path):
    info = path.stat(follow_symlinks=False)
    if not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
        raise ValueError('package executable/script is not a trusted regular file')
    return str(path)


def wait_socket(path, seconds=30):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            info = os.stat(path, follow_symlinks=False)
            if stat.S_ISSOCK(info.st_mode) and info.st_uid == 0 and not info.st_mode & 0o077:
                return
        except FileNotFoundError:
            pass
        time.sleep(0.1)
    raise RuntimeError('required package socket is not ready')


def command(action, config):
    bins = Path(config['BIN_DIR'])
    scripts = Path(config['LIBEXEC_DIR'])
    profile = config['PROFILE']
    control = profile == 'vkms' or config['CONTROL'] == 'x11'
    if action == 'capture':
        ffmpeg = Path(config['FFMPEG'])
        trusted_file(ffmpeg)
        if not ffmpeg.stat(follow_symlinks=False).st_mode & 0o111:
            raise ValueError('configured FFmpeg is not executable')
        if profile == 'x11':
            device, _ = drm_paths(config)
            return ['/bin/bash', trusted_file(scripts / 'start-console-x11.sh'), str(ffmpeg), device]
        device, _ = drm_paths(config)
        return [trusted_file(bins / 'dwconsole-daemon'), '--socket', CAPTURE,
                '--device', device, '--fps', '30', '--encoder', 'libx264', '--ffmpeg', str(ffmpeg)]
    if action == 'broker':
        return [trusted_file(bins / 'socket-broker'), '--socket', BROKER, '--capture-socket', CAPTURE]
    if action == 'input':
        if not control:
            raise ValueError('input requires VKMS or explicit guarded X11 control')
        return [trusted_file(bins / 'input-daemon'), '--socket', INPUT, '--width', config['WIDTH'],
                '--height', config['HEIGHT'], '--guard-state', GUARD]
    if action == 'session-supervisor':
        if config['CONTROL'] == 'x11':
            args = ['/usr/bin/python3', trusted_file(scripts / 'x11-session-supervisor.py'),
                    '--libexec-dir', str(scripts), '--width', config['WIDTH'], '--height', config['HEIGHT']]
            if profile == 'kms':
                _, state = drm_paths(config)
                args += ['--drm-state', state]
            return args
        if profile != 'vkms':
            return None
        _, state = drm_paths(config)
        if not Path(state).is_file():
            raise ValueError('VKMS input guard requires readable DRM debug state')
        return ['/usr/bin/python3', trusted_file(scripts / 'session-supervisor.py'),
                '--host-ip', config['LISTEN_IP'], '--bin-dir', str(bins), '--libexec-dir', str(scripts),
                '--assets', config['ASSETS'], '--drm-state', state,
                '--capture-socket', CAPTURE, '--input-socket', INPUT, '--guard-state', GUARD,
                '--broker-socket', BROKER]
    if action == 'web':
        wait_socket(CAPTURE)
        wait_socket(BROKER)
        args = [trusted_file(bins / 'web-launch'), '--socket', CAPTURE,
                '--capture-broker-socket', BROKER, '--executable', trusted_file(bins / 'console-web'),
                '--uid', '65534', '--gid', '65534']
        if control:
            wait_socket(INPUT)
            args += ['--input-socket', INPUT]
        address = config['LISTEN_IP']
        args += ['--', '--stream-fd', '0', '--capture-broker-fd', '4', '--listen', address + ':8090',
                 '--ice-listen', address + ':8091', '--origin', 'http://' + address + ':8090',
                 '--assets', config['ASSETS']]
        if control:
            args += ['--input-fd', '3']
        return args
    raise ValueError('unknown package runtime action')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('capture', 'broker', 'input', 'web', 'session-supervisor', 'check-config'))
    args = parser.parse_args()
    if os.geteuid() != 0:
        parser.error('package runtime requires root before launcher privilege drop')
    config = load_config()
    if args.action == 'check-config':
        print('configuration valid; profile=' + config['PROFILE'])
        return
    executable = command(args.action, config)
    if executable is not None:
        os.execv(executable[0], executable)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError) as error:
        print('package runtime unavailable: ' + str(error), file=sys.stderr)
        sys.exit(1)
