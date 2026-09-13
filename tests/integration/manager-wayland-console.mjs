import {chromium,expect} from '@playwright/test';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import {createHash,X509Certificate} from 'node:crypto';
const base='https://192.168.10.11:30443';
const device='fd6b6a59-40fc-41ac-84ef-b498ca8d6880';
const home=`${base}/?device=${device}&managed`;
const output='artifacts/manager-wayland-console';mkdirSync(output,{recursive:true});
const cert=new X509Certificate(readFileSync('/home/localuser/.local/state/dwdesktop-manager-preview/pki/server.crt'));
const pin=createHash('sha256').update(cert.publicKey.export({type:'spki',format:'der'})).digest('base64');
const browser=await chromium.launch({executablePath:'/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome',args:[`--ignore-certificate-errors-spki-list=${pin}`]});
const page=await browser.newPage({viewport:{width:1280,height:900}});
const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.addInitScript(()=>{const Original=window.RTCPeerConnection;window.__peers=[];window.__channels=[];window.RTCPeerConnection=class extends Original{constructor(...args){super(...args);window.__peers.push(this)}createDataChannel(...args){const dc=super.createDataChannel(...args);window.__channels.push(dc);return dc}}});
async function videoReady(){await page.waitForFunction(()=>document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames>30 && !document.querySelector('.console-placeholder'),null,{timeout:45000})}
async function point(x,y){return page.locator('video').evaluate((v,{x,y})=>{const b=v.getBoundingClientRect(),s=Math.min(b.width/v.videoWidth,b.height/v.videoHeight);return{x:b.x+(b.width-v.videoWidth*s)/2+x*s,y:b.y+(b.height-v.videoHeight*s)/2+y*s}},{x,y})}
try {
 await page.goto(home);
 const card=page.locator('.broker-card').filter({hasText:'Existing Wayland console'});
 await expect(card.getByRole('button',{name:'Open console'})).toBeEnabled({timeout:20000});
 await expect(card.getByRole('button',{name:'End desktop'})).toHaveCount(0);
 await card.getByRole('button',{name:'Open console'}).click();
 await videoReady();
 await expect(page.getByRole('button',{name:'4K',exact:true})).toHaveCount(0);
 await page.screenshot({path:`${output}/before.png`});
 console.log('Wayland console video decoded',page.url());
 await page.locator('video').click();
 await expect(page.locator('.console-view')).toHaveAttribute('data-input-state','controlling',{timeout:5000});
 // Real browser input only: open Activities, search for Calculator, launch.
 const activities=await point(30,15);await page.mouse.click(activities.x,activities.y);
 await page.keyboard.type('calculator',{delay:90});await page.waitForTimeout(1200);
 await page.keyboard.press('Enter');await page.waitForTimeout(2500);
 // Type a calculation using physical key events through the same channel.
 const calculator=await point(270,180);await page.mouse.click(calculator.x,calculator.y);
 await page.keyboard.press('Escape');await page.keyboard.type('123',{delay:90});
 await page.keyboard.down('Shift');await page.keyboard.press('Equal');await page.keyboard.up('Shift');
 await page.keyboard.type('456',{delay:90});await page.keyboard.press('Enter');await page.waitForTimeout(800);
 await page.screenshot({path:`${output}/calculator.png`});
 const gateway=await page.evaluate(async()=>{const pc=window.__peers.findLast(p=>p.connectionState==='connected');const stats=await pc.getStats();let remote;stats.forEach(s=>{if(s.type==='transport'&&s.selectedCandidatePairId){const pair=stats.get(s.selectedCandidatePairId);remote=stats.get(pair.remoteCandidateId)}});return remote&&{address:remote.address,port:remote.port}});
 if(gateway?.address!=='192.168.10.11'||gateway?.port!==30445)throw Error('Incorrect media route');
 await page.reload();await videoReady();
 await page.locator('video').click();await expect(page.locator('.console-view')).toHaveAttribute('data-input-state','controlling',{timeout:5000});
 await page.screenshot({path:`${output}/reconnected.png`});
 await page.getByRole('button',{name:'Release input',exact:true}).click();
 // Transport-level negative test, separate from the real desktop interaction:
 // unrenewed grants expire and out-of-order events cannot be replayed.
 const leaseSafety=await page.evaluate(async()=>{
  const dc=window.__channels.findLast(c=>c.readyState==='open');
  const next=(send)=>new Promise((resolve,reject)=>{const timer=setTimeout(()=>{dc.removeEventListener('message',receive);reject(Error('input response timeout'))},2500);const receive=e=>{clearTimeout(timer);dc.removeEventListener('message',receive);resolve(JSON.parse(e.data))};dc.addEventListener('message',receive);send?.()});
  await new Promise(r=>setTimeout(r,200));
  const grant=await next(()=>dc.send(JSON.stringify({type:'acquire'})));
  if(grant.type!=='ready')throw Error('lease acquisition failed');
  const expired=await next();if(expired.type!=='unavailable')throw Error('lease did not expire');
  const grant2=await next(()=>dc.send(JSON.stringify({type:'acquire'})));
  const rejected=await next(()=>dc.send(JSON.stringify({type:'event',generation:grant2.generation,sequence:2,event:{type:'renew'}})));
  if(rejected.type!=='unavailable')throw Error('out-of-order input accepted');
  return {expiry:true,outOfOrderRejected:true};
 });
 if(errors.length)throw Error(errors.join('\n'));
 writeFileSync(`${output}/report.json`,JSON.stringify({url:page.url(),gateway,video:true,reconnect:true,leaseSafety,input:'calculator driven via browser; screenshot requires visual inspection',jsErrors:errors},null,2));
 console.log('Console reconnect/video/control passed',JSON.stringify(gateway));
} catch(error) {await page.screenshot({path:`${output}/failure.png`}).catch(()=>{});console.log(await page.locator('body').innerText());throw error}
finally {await browser.close()}
