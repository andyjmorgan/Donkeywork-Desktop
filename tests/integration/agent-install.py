"""Attic preview enrollment/connectivity plus scoped local systemd install proof."""
import json
import os
from pathlib import Path
import ssl
import subprocess
import tarfile
import tempfile
import time
import urllib.error
import urllib.request

base = 'https://192.168.10.11:30443'
ca = Path('/home/localuser/.local/state/dwdesktop-manager-preview/pki/preview-ca.crt')
context = ssl.create_default_context(cafile=str(ca))
root = Path(tempfile.mkdtemp(prefix='dwdesktop-agent-install-'))
def api(path, body=None):
    request = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(), headers={'Origin': base, 'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, context=context, timeout=15) as response:
        return json.load(response)
def online(identifier, wanted):
    for _ in range(60):
        try:
            device = next(d for d in api('/api/v1/devices')['devices'] if d['id'] == identifier)
            if device.get('online', False) == wanted:
                return
        except (urllib.error.URLError, StopIteration):
            pass
        time.sleep(1)
    raise RuntimeError('device presence did not reach expected state')
def run(args):
    result = subprocess.run(args, capture_output=True, text=True)
    if result.returncode:
        # Do not print args: the explicitly tested --code form contains a secret.
        raise RuntimeError('device operation failed: ' + result.stderr[-500:])
    return result.stdout

with urllib.request.urlopen(base+'/downloads/dwdesktop-agent-0.1.0-preview3.tar.gz',context=context) as response:
    archive=root/'package.tar.gz';archive.write_bytes(response.read(32*1024*1024))
with tarfile.open(archive) as archive_file:
    archive_file.extractall(root, filter='data')
binary=str(root/'dwdesktop-agent-amd64')
registration=api('/api/v1/devices',{'name':'Agent connectivity verification','description':'Temporary isolated connectivity proof; removed after revocation.'})
identifier=registration['device']['id']
state=root/'state'
run([binary,'enroll','--state-dir',str(state),'--manager',base,'--code',registration['code'],'--ca-file',str(ca)])
process=None
try:
    process=subprocess.Popen([binary,'run','--state-dir',str(state)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    online(identifier,True)
    process.terminate();process.wait(timeout=10);online(identifier,False)
    process=subprocess.Popen([binary,'run','--state-dir',str(state)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    online(identifier,True)
    api('/api/v1/devices/'+identifier+'/revoke',{})
    online(identifier,False)
    time.sleep(4);online(identifier,False)
finally:
    if process and process.poll() is None:process.terminate();process.wait(timeout=10)
api('/api/v1/devices/'+identifier+'/delete',{})
print('Isolated enrollment, online/offline, reconnect and revocation passed. Test record removed.')

if subprocess.check_output(['hostname'],text=True).strip()!='easternkingdoms':
    raise RuntimeError('local install scope is Easternkingdoms only')
if subprocess.run(['sudo','-n','test','-e','/var/lib/dwdesktop-agent/device.json']).returncode==0:
    raise RuntimeError('existing device install: refusing to replace its identity')
registration=api('/api/v1/devices',{'name':'Easternkingdoms device service','description':'Ubuntu host · certificate-authenticated outbound agent. Desktop pilots untouched.'})
identifier=registration['device']['id']
run(['sudo','-n',str(root/'install.sh'),'--manager',base,'--code',registration['code'],'--ca-file',str(ca)])
online(identifier,True)
before=run(['sudo','-n','sha256sum','/var/lib/dwdesktop-agent/device.crt'])
run(['sudo','-n',str(root/'install.sh'),'--manager',base])
online(identifier,True)
after=run(['sudo','-n','sha256sum','/var/lib/dwdesktop-agent/device.crt'])
if before!=after:raise RuntimeError('reinstallation replaced device identity')
print(json.dumps({'installedHost':'easternkingdoms','deviceId':identifier,'headlessCodeArgument':True,'online':True,'reinstallPreservesIdentity':True}))
