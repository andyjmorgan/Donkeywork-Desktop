import { execFileSync } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { randomUUID } from 'node:crypto';
import { readFileSync } from 'node:fs';
const host = process.argv[2];
if (!['192.168.69.21','192.168.69.28','192.168.69.17'].includes(host)) throw Error('pilot required');
const base = `http://${host}:8095`;
const list = async () => (await (await fetch(base+'/api/desktops')).json()).desktops;
const original = (await list()).find(d=>d.environment==='gnome' && d.state==='ready');
if (!original) throw Error('existing GNOME required');
const local = host === '192.168.69.17' && execFileSync('hostname',{encoding:'utf8'}).trim() === 'easternkingdoms';
const pid = id => local
  ? execFileSync('systemctl',['--user','show',`dwdesktop-session-${id}`,'-p','MainPID','--value'],{encoding:'utf8'}).trim()
  : execFileSync('ssh',[host,`systemctl --user show dwdesktop-session-${id} -p MainPID --value`],{encoding:'utf8'}).trim();
const originalPid = pid(original.id);
const create = () => fetch(base+'/api/desktops',{method:'POST',headers:{Origin:base,'Content-Type':'application/json'},body:JSON.stringify({environment:'gnome',requestId})});
const requestId = randomUUID();
const response = await create();
if (response.status!==202) throw Error('create failed');
const added = (await response.json()).desktop;
if ((await (await create()).json()).desktop.id !== added.id) throw Error('idempotency failed');
for (let n=0;n<110;n++) {
  const desktops=await list();
  if (!desktops.some(d=>d.id===original.id) || !desktops.some(d=>d.id===added.id)) throw Error('desktop lost');
  await delay(1000);
}
if (!(await list()).some(d=>d.id===added.id&&d.state==='ready')) throw Error('new desktop not ready');
if (local) execFileSync('python3',['tests/integration/managed-broker-logout.py',added.id]);
else execFileSync('ssh',[host,`python3 - ${added.id}`],{input:readFileSync('tests/integration/managed-broker-logout.py')});
for(let n=0;n<20 && (await list()).some(d=>d.id===added.id);n++) await delay(1000);
if ((await list()).some(d=>d.id===added.id)) throw Error('logout did not retire desktop');
if (pid(original.id)!==originalPid) throw Error('sibling replaced');
await delay(3000);
if ((await list()).some(d=>d.id!==original.id)) throw Error('unexpected desktop after logout');
console.log(JSON.stringify({host,concurrentGnome:true,idempotentCreate:true,normalLogout:true,noAutomaticReplacement:true,survivingDesktop:original.id}));
