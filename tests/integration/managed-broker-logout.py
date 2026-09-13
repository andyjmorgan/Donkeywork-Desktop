"""Normal GNOME logout using only the named managed desktop's private bus."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys

identifier = sys.argv[1]
if not re.fullmatch('[0-9a-f]{32}', identifier):
    raise RuntimeError('invalid desktop ID')
root = Path.home() / '.local/state/dwdesktop-managed/sessions' / identifier
metadata = json.loads((root / 'instance.json').read_text())
unit = 'dwdesktop-session-' + identifier + '.service'
group = subprocess.check_output(['systemctl', '--user', 'show', unit, '-p', 'ControlGroup', '--value'], text=True).strip()
if not group.endswith('/' + unit):
    raise RuntimeError('unexpected cgroup')
for pid in (Path('/sys/fs/cgroup') / group.lstrip('/') / 'cgroup.procs').read_text().split():
    proc = Path('/proc') / pid
    try:
        command = proc.joinpath('cmdline').read_bytes().split(b'\0')[0]
        if not command.endswith(b'/gnome-session-binary'):
            continue
        env = dict(item.decode().split('=', 1) for item in proc.joinpath('environ').read_bytes().split(b'\0') if b'=' in item)
        if env.get('DISPLAY') != ':' + str(metadata['display']):
            raise RuntimeError('wrong display')
        subprocess.run(['gnome-session-quit', '--logout', '--no-prompt'], env=env, check=True, timeout=15)
        break
    except FileNotFoundError:
        continue
else:
    raise RuntimeError('session manager not found')
