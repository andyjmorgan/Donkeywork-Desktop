"""Bind guarded input to the existing package-captured X11 seat. No display writes."""
import argparse
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import threading
import time

from x11_support import discover_target

GUARD = Path('/run/donkeywork-desktop-guard/state.json')
GUARD_UNIT = 'donkeywork-desktop-input-guard.service'
CAPTURE_UNIT = 'donkeywork-desktop-capture.service'


def command(args, check=True):
    return subprocess.run(args, check=check, stdin=subprocess.DEVNULL,
                          stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                          text=True, timeout=8).stdout.strip()


def capture_pid():
    group = command(['systemctl', 'show', CAPTURE_UNIT, '-p', 'ControlGroup', '--value'])
    if not group.startswith('/system.slice/') or '..' in Path(group).parts:
        raise ValueError('capture cgroup unavailable')
    candidates = []
    # VKMS/KMS captures use dwconsole-daemon; the Spark X11 profile uses the
    # host FFmpeg process launched by start-console-x11.sh.  Both are trusted
    # package capture producers and must be accepted for guard binding.
    wrapper_root = Path('/opt/donkeywork-desktop/releases').resolve(strict=True)
    ffmpeg = Path('/usr/bin/ffmpeg').resolve(strict=True)
    text = (Path('/sys/fs/cgroup') / group.lstrip('/') / 'cgroup.procs').read_text()
    if len(text) > 65536:
        raise ValueError('capture cgroup too large')
    for value in text.splitlines():
        if not value.isdigit():
            raise ValueError('invalid process ID')
        process = Path('/proc') / value
        try:
            if process.stat().st_uid == 0:
                executable = (process / 'exe').resolve(strict=True)
                is_wrapper = (executable.name == 'dwconsole-daemon'
                              and wrapper_root in executable.parents)
                if is_wrapper:
                    candidates.insert(0, int(value))
                elif executable == ffmpeg:
                    candidates.append(int(value))
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
    # When the wrapper supervises FFmpeg, use the wrapper as the stable
    # identity for X11 metadata; otherwise accept a lone FFmpeg capture.
    if candidates and (Path('/proc') / str(candidates[0]) / 'exe').resolve(strict=True).name == 'dwconsole-daemon':
        return candidates[0]
    if len(candidates) != 1:
        raise ValueError('one package capture process required')
    return candidates[0]


def valid_guard(session, width, height, since=0):
    try:
        info = GUARD.stat(follow_symlinks=False)
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022
                or info.st_size > 4096 or info.st_mtime < since
                or not 0 <= time.time() - info.st_mtime <= 0.5):
            return False
        data = json.loads(GUARD.read_text())
        return (data.get('valid') is True and data.get('width') == width
                and data.get('height') == height
                and str(data.get('session', '')).startswith(session + ':'))
    except (OSError, ValueError):
        return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--libexec-dir', type=Path, required=True)
    parser.add_argument('--width', type=int, required=True)
    parser.add_argument('--height', type=int, required=True)
    parser.add_argument('--drm-state')
    args = parser.parse_args()
    if os.geteuid() != 0:
        parser.error('root required')
    helper = args.libexec_dir / 'x11-target-guard.py'
    info = helper.stat(follow_symlinks=False)
    if not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
        parser.error('untrusted guard helper')
    stop = threading.Event()
    signal.signal(signal.SIGTERM, lambda *_: stop.set())
    signal.signal(signal.SIGINT, lambda *_: stop.set())
    identity = None
    notice = None
    try:
        while not stop.is_set():
            try:
                pid = capture_pid()
                target = discover_target(pid, args.width, args.height, args.drm_state)
                current = (pid, target['session'], target['uid'], target['display'], target['xauthority'])
                if current != identity or not valid_guard(target['session'], args.width, args.height):
                    command(['systemctl', 'stop', GUARD_UNIT], check=False)
                    command(['systemctl', 'reset-failed', GUARD_UNIT], check=False)
                    flags = ['--session', target['session'], '--session-uid', str(target['uid']),
                             '--display', target['display'], '--xauthority', target['xauthority'],
                             '--capture-pid', str(pid), '--width', str(args.width),
                             '--height', str(args.height), '--output', str(GUARD)]
                    if args.drm_state:
                        flags += ['--drm-state', args.drm_state]
                    since = time.time()
                    command(['systemd-run', '--quiet', '--collect', '--unit=' + GUARD_UNIT,
                             '--property=PartOf=donkeywork-desktop.target',
                             '--property=RuntimeDirectory=donkeywork-desktop-guard',
                             '--property=RuntimeDirectoryMode=0700', '--property=NoNewPrivileges=yes',
                             '--', '/usr/bin/python3', str(helper)] + flags)
                    deadline = time.monotonic() + 5
                    while not valid_guard(target['session'], args.width, args.height, since):
                        if stop.wait(0.1) or time.monotonic() >= deadline:
                            raise RuntimeError('guard not ready')
                    command(['systemctl', 'start', 'donkeywork-desktop-input.service'])
                    command(['systemctl', 'start', 'donkeywork-desktop-web.service'])
                    identity = current
                    print('X11 console guard and input ready', flush=True)
                    notice = None
                stop.wait(0.25)
            except Exception as error:
                command(['systemctl', 'stop', GUARD_UNIT], check=False)
                identity = None
                if notice != type(error).__name__:
                    notice = type(error).__name__
                    print('X11 input waiting for verified capture (' + notice + ')', flush=True)
                stop.wait(1)
    finally:
        command(['systemctl', 'stop', GUARD_UNIT], check=False)


if __name__ == '__main__':
    main()
