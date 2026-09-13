#!/usr/bin/python3
"""Root-only freshness guard for an existing X11 console. Never injects input."""
import argparse
import json
import os
from pathlib import Path
import signal
import stat
import tempfile
import threading
import time
import uuid

from x11_support import active_session, authority_identity, capture_identity, drm_snapshot, randr_snapshot


def snapshot(args):
    session = active_session()
    if session['session'] != args.session or session['uid'] != args.session_uid:
        raise ValueError('active X11 session changed')
    auth = authority_identity(args.xauthority, args.session_uid)
    capture = capture_identity(args.capture_pid, args.display, args.xauthority,
                               args.width, args.height, args.drm_state)
    layout = randr_snapshot(args.display, args.xauthority, args.width, args.height, args.drm_state is not None)
    drm = drm_snapshot(args.drm_state, args.width, args.height, layout[4]) if args.drm_state else None
    # A lock transition is a new target even though the X server may be unchanged.
    return (session['session'], session['uid'], session['vt'], session['locked'], auth, capture, layout, drm)


def publish(path, identity, width, height, valid):
    descriptor, temporary = tempfile.mkstemp(prefix='.x11-guard-', dir=path.parent)
    try:
        with os.fdopen(descriptor, 'w') as output:
            json.dump({'session': identity, 'width': width, 'height': height, 'valid': valid}, output)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--session', required=True)
    parser.add_argument('--session-uid', type=int, required=True)
    parser.add_argument('--display', required=True)
    parser.add_argument('--xauthority', required=True)
    parser.add_argument('--capture-pid', type=int, required=True)
    parser.add_argument('--width', type=int, default=1920)
    parser.add_argument('--height', type=int, default=1080)
    parser.add_argument('--drm-state', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--check', action='store_true', help='one read-only target check; no guard file publication')
    args = parser.parse_args()
    if os.geteuid() != 0 or args.session_uid <= 0 or args.capture_pid <= 1 or not 2 <= args.width <= 4096 or not 2 <= args.height <= 4096:
        parser.error('root and a valid active account/capture target are required')
    if args.check:
        state = snapshot(args)
        print(json.dumps({'valid': True, 'session': args.session, 'display': args.display,
                          'width': args.width, 'height': args.height, 'capturePid': args.capture_pid,
                          'output': state[6][0]}))
        return
    if args.output is None or not args.output.is_absolute():
        parser.error('absolute --output required unless --check')
    parent = args.output.parent.stat(follow_symlinks=False)
    if not stat.S_ISDIR(parent.st_mode) or parent.st_uid != 0 or parent.st_mode & 0o077:
        parser.error('guard output directory must be root-private')
    stop = threading.Event()
    signal.signal(signal.SIGTERM, lambda *_: stop.set())
    signal.signal(signal.SIGINT, lambda *_: stop.set())
    identity = args.session + ':x11:' + uuid.uuid4().hex
    publish(args.output, identity, args.width, args.height, False)
    try:
        baseline = snapshot(args)
        print('X11 guard validated one unscaled output and current capture process', flush=True)
        while not stop.is_set():
            started = time.monotonic()
            try:
                valid = snapshot(args) == baseline
            except Exception:
                valid = False
            publish(args.output, identity, args.width, args.height, valid)
            if not valid:
                print('X11 target changed or unavailable; input remains invalid until revalidated', flush=True)
                break
            stop.wait(max(0, 0.1 - (time.monotonic() - started)))
    finally:
        publish(args.output, identity, args.width, args.height, False)


if __name__ == '__main__':
    main()
