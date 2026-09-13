#!/usr/bin/env python3
"""Enroll an existing Ubuntu session pilot; never install/restart a desktop."""
import argparse
import json
from pathlib import Path
import shlex
import ssl
import subprocess
import time
import urllib.request

p=argparse.ArgumentParser()
p.add_argument('host',choices=['192.168.69.21','192.168.69.28'])
a=p.parse_args()
name={'192.168.69.21':'Minigpu','192.168.69.28':'Spark'}[a.host]
repo=Path(__file__).resolve().parents[2]
ca=Path('/home/localuser/.local/state/dwdesktop-manager-preview/pki/preview-ca.crt')
base='https://192.168.10.11:30443'
tls=ssl.create_default_context(cafile=str(ca))
def remote(command,data=None):
 r=subprocess.run(['ssh','-o','BatchMode=yes',a.host,command],input=data,text=True,capture_output=True,timeout=90)
 if r.returncode:raise RuntimeError('Pilot operation failed: '+r.stderr[-700:])
 return r.stdout.strip()
def upload(source,target):
 subprocess.run(['scp','-q',str(source),a.host+':'+target],check=True,timeout=60)
def api(path,body=None):
 req=urllib.request.Request(base+path,data=None if body is None else json.dumps(body).encode(),headers={'Origin':base,'Content-Type':'application/json'})
 with urllib.request.urlopen(req,context=tls,timeout=20) as r:return json.load(r)

remote('sudo -n true && test -f /home/localuser/.local/state/dwdesktop-managed/broker.py && test -x /home/localuser/.local/state/dwdesktop-managed/bin/dwconsole-web')
before=remote("systemctl --user show 'dwdesktop-session-*.service' -p Id -p MainPID")
tmp=remote('mktemp -d /tmp/dwdesktop-manager-rollout.XXXXXX')
q=shlex.quote
upload(ca,tmp+'/manager-ca.crt')
upload(repo/'deploy/managed/broker.py',tmp+'/broker.py')
upload(repo/'deploy/managed/manager-bridge.tmpfiles.conf',tmp+'/tmpfiles.conf')
remote(f'curl --fail --silent --show-error --cacert {q(tmp)}/manager-ca.crt {base}/downloads/dwdesktop-agent-0.1.0-preview6.tar.gz -o {q(tmp)}/package.tar.gz && tar -xzf {q(tmp)}/package.tar.gz -C {q(tmp)}')
enrolled=remote('if sudo -n test -e /var/lib/dwdesktop-agent/device.json; then echo yes; else echo no; fi')=='yes'
code=None
if not enrolled:
 issued=api('/api/v1/devices',{'name':name,'description':f'Ubuntu 24.04 · {a.host} · managed GNOME/Xfce desktops'})
 code=issued['code']
command=f'sudo -n bash {q(tmp)}/install.sh --manager {base} --ca-file {q(tmp)}/manager-ca.crt'
if code is not None:command+=' --code-file /dev/stdin'
remote(command,code)
del code
identity=json.loads(remote('sudo -n cat /var/lib/dwdesktop-agent/device.json'))['deviceId']
remote(f'sudo -n install -m 0644 {q(tmp)}/tmpfiles.conf /etc/tmpfiles.d/dwdesktop-session.conf && sudo -n systemd-tmpfiles --create /etc/tmpfiles.d/dwdesktop-session.conf')
remote(f'install -m 0600 /home/localuser/.local/state/dwdesktop-managed/broker.py {q(tmp)}/previous-broker.py')
remote('systemctl --user stop dwdesktop-managed-web.service')
remote(f'install -m 0700 {q(tmp)}/broker.py /home/localuser/.local/state/dwdesktop-managed/broker.py')
remote('systemd-run --user --collect --unit=dwdesktop-managed-web.service --property=KillMode=control-group --property=TimeoutStopSec=10 --property=Restart=always --property=RestartSec=2 python3 /home/localuser/.local/state/dwdesktop-managed/broker.py /home/localuser/.local/state/dwdesktop-managed --host '+a.host+' --socket /run/dwdesktop-session/broker.sock')
for _ in range(45):
 d=next(d for d in api('/api/v1/devices')['devices'] if d['id']==identity)
 if d.get('online') and d.get('broker'):break
 time.sleep(1)
else:raise RuntimeError('Device broker did not become available')
inventory=api(f'/api/v1/devices/{identity}/broker/desktops')
after=remote("systemctl --user show 'dwdesktop-session-*.service' -p Id -p MainPID")
if before!=after:raise RuntimeError('Existing desktop unit inventory changed; inspect before claiming preservation')
print(json.dumps({'host':a.host,'name':name,'deviceId':identity,'online':True,'broker':True,'existingDesktopsPreserved':True,'desktopCount':len(inventory['desktops']),'url':base+'/?device='+identity+'&managed'}))
