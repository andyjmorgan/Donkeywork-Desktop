import {chromium,expect} from '@playwright/test';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import {createHash,X509Certificate} from 'node:crypto';
const base='https://192.168.10.11:30443';
const targets={easternkingdoms:'16df48d0-3827-4dc0-963e-04a8f65335e5',minigpu:'fd6b6a59-40fc-41ac-84ef-b498ca8d6880',spark:'b9f6b90d-38f3-45fa-abc9-2297a827dffb'};
const target=process.argv[2]??'easternkingdoms';
const device=targets[target];if(!device)throw Error('Explicit enrolled pilot required');
const api=`/api/v1/devices/${device}/broker`;
const home=`${base}/?device=${device}&managed&view=terminal`;
const output=`artifacts/manager-session-proof/${target}`;mkdirSync(output,{recursive:true});
const cert=new X509Certificate(readFileSync('/home/localuser/.local/state/dwdesktop-manager-preview/pki/server.crt'));
const pin=createHash('sha256').update(cert.publicKey.export({type:'spki',format:'der'})).digest('base64');
const browser=await chromium.launch({executablePath:'/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome',headless:true,args:[`--ignore-certificate-errors-spki-list=${pin}`]});
let created;
const page=await browser.newPage({viewport:{width:1440,height:1050}});
page.setDefaultTimeout(15000);
const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.addInitScript(()=>{
 const Original=window.RTCPeerConnection;
 window.__peers=[];
 window.RTCPeerConnection=class extends Original{constructor(...args){super(...args);window.__peers.push(this)}};
});
const request=async(path,body)=>page.evaluate(async({path,body})=>{
 const r=await fetch(path,body===undefined?{}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});
 return {status:r.status,body:await r.json()};
},{path,body});
const inventory=async()=>(await request(`${api}/desktops`)).body.desktops;
async function videoReady(){await page.waitForFunction(()=>document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames>30,null,{timeout:45000})}
try{
 await page.goto(home);
 await expect(page.getByRole('navigation',{name:'Breadcrumb'}).getByRole('link',{name:'Fleet console'})).toHaveAttribute('href','/');
 await expect(page.getByRole('region',{name:'Connection details'})).toContainText(target==='spark'?'Spark':target==='minigpu'?'Minigpu':'Easternkingdoms',{timeout:15000});
 console.log('Broker page loaded');
 await expect(page.getByRole('button',{name:'Create desktop',exact:true})).toBeEnabled({timeout:45000});
 const before=await inventory();
 console.log('Inventory loaded');
 await page.getByLabel('Desktop environment',{exact:true}).selectOption('gnome');
 console.log('GNOME selected');
 await page.getByRole('button',{name:'Create desktop',exact:true}).click();
 console.log('Create clicked');
 await expect.poll(async()=>{
  const fresh=(await inventory()).filter(d=>!before.some(old=>old.id===d.id));
  if(fresh.length===1)created=fresh[0].id;
  return Boolean(created);
 },{timeout:20000}).toBe(true);
 console.log('Created isolated test desktop',created);
 await expect.poll(async()=>(await inventory()).find(d=>d.id===created)?.state,{timeout:60000}).toBe('ready');
 console.log('Desktop ready; opening viewer');
 await page.locator(`[data-desktop-id="${created}"]`).getByRole('button',{name:'Reconnect',exact:true}).click();
 await videoReady();
 console.log('Video decoded');
 await expect(page.getByRole('region',{name:'Connection details'})).toContainText('gnome · localuser',{timeout:15000});
 const video=page.locator('video');await video.click();
 await expect(page.locator('.console-view')).toHaveAttribute('data-input-state','controlling',{timeout:10000});
 const activities=await video.evaluate(v=>{const b=v.getBoundingClientRect(),s=Math.min(b.width/v.videoWidth,b.height/v.videoHeight);return {x:b.x+(b.width-v.videoWidth*s)/2+30*s,y:b.y+(b.height-v.videoHeight*s)/2+15*s}});
 await page.mouse.click(activities.x,activities.y);await page.keyboard.type('Text Editor',{delay:100});await page.waitForTimeout(1500);await page.keyboard.press('Enter');await page.waitForTimeout(2000);
 // Wait for the actual app, then focus its text area. Lowercase physical keys
 // avoid Playwright's synthesized shifted-character events, unlike real Shift.
 await page.waitForTimeout(2500);
 const editor=await video.evaluate(v=>{const b=v.getBoundingClientRect(),s=Math.min(b.width/v.videoWidth,b.height/v.videoHeight);return{x:b.x+(b.width-v.videoWidth*s)/2+600*s,y:b.y+(b.height-v.videoHeight*s)/2+150*s}});
 await page.mouse.click(editor.x,editor.y);await page.keyboard.press('Control+a');
 await page.keyboard.type('manager bridge keyboard and mouse through the gateway',{delay:70});await page.waitForTimeout(700);
 await page.screenshot({path:`${output}/gnome-input.png`});
 const selected=await page.evaluate(async()=>{
  const pc=window.__peers.findLast(p=>p.connectionState==='connected');if(!pc)return null;
  const stats=await pc.getStats();let result;
  stats.forEach(s=>{if(s.type==='transport'&&s.selectedCandidatePairId){const pair=stats.get(s.selectedCandidatePairId);result=stats.get(pair.remoteCandidateId)}});
  return result&&{address:result.address,port:result.port,protocol:result.protocol};
 });
 if(selected?.address!=='192.168.10.11'||selected?.port!==30445)throw Error('Browser did not select the manager media gateway: '+JSON.stringify(selected));
 await page.reload();await videoReady();
 await page.getByRole('button',{name:'4K',exact:true}).click();
 await page.waitForFunction(()=>document.querySelector('video')?.videoWidth===3840,null,{timeout:45000});
 await expect(page.getByRole('button',{name:'1080p',exact:true})).toBeEnabled({timeout:15000});
 await page.getByRole('button',{name:'1080p',exact:true}).click();
 await page.waitForFunction(()=>document.querySelector('video')?.videoWidth===1920,null,{timeout:45000});
 await videoReady();await page.screenshot({path:`${output}/resized-1080p.png`});
 await page.getByRole('link',{name:'All desktops',exact:true}).click();
 await expect(page).toHaveURL(home);
 page.once('dialog',d=>d.accept());
 await page.locator(`[data-desktop-id="${created}"]`).getByRole('button',{name:'End desktop',exact:true}).click();
 await expect.poll(async()=>(await inventory()).some(d=>d.id===created),{timeout:20000}).toBe(false);
 const after=await inventory();if(before.some(d=>!after.some(a=>a.id===d.id)))throw Error('Existing desktop disappeared');
 await page.getByRole('navigation',{name:'Breadcrumb'}).getByRole('link',{name:'Fleet console'}).click();
 await expect(page).toHaveURL(base+'/');
 await expect(page.locator(`[data-device-id="${device}"]`).getByRole('link',{name:'Open desktops',exact:true})).toBeVisible({timeout:15000});
 if(errors.length)throw Error(errors.join('\n'));
 const report={device,created,create:true,reconnect:true,input:'screenshot requires visual review',resize:'1080p -> 4K -> 1080p',destroy:true,existingDesktopsPreserved:true,mediaGateway:selected,jsErrors:errors};
 writeFileSync(`${output}/report.json`,JSON.stringify(report,null,2));console.log(JSON.stringify(report));
}catch(e){await page.screenshot({path:`${output}/failure.png`}).catch(()=>{});throw e}
finally{
 if(created){await request(`${api}/desktops/${created}/close`,{}).catch(()=>{})}
 await browser.close();
}
