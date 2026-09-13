"""Explicitly rotate only this preview database's generated password, without logs."""
import base64
import json
import secrets
import subprocess

base = ['kubectl', '--context', 'attic', '-n', 'donkeywork-desktop']
secret = json.loads(subprocess.check_output(base + ['get', 'secret', 'manager-secrets', '-o', 'json']))
password = secrets.token_hex(32)  # No trailing newline; no shell interpolation.
subprocess.run(base + ['exec', '-i', 'deploy/postgres', '--', 'psql', '-U', 'desktops', '-d', 'desktops', '-v', 'ON_ERROR_STOP=1'],
               input="ALTER ROLE desktops PASSWORD '" + password + "';\n", text=True, capture_output=True, check=True)
secret['data']['postgres-password'] = base64.b64encode(password.encode()).decode()
subprocess.run(base + ['replace', '-f', '-'], input=json.dumps(secret), text=True, capture_output=True, check=True)
subprocess.run(base + ['rollout', 'restart', 'deployment/manager'], check=True)
print('Preview database password rotated; manager restarting.')
