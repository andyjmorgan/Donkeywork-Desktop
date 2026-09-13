import {chromium,expect} from '@playwright/test';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import {createHash,X509Certificate} from 'node:crypto';
const base='https://192.168.10.11:30443';
const cert=new X509Certificate(readFileSync('/home/localuser/.local/state/dwdesktop-manager-preview/pki/server.crt'));
const pin=createHash('sha256').update(cert.publicKey.export({type:'spki',format:'der'})).digest('base64');
const browser=await chromium.launch({executablePath:'/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome',args:[`--ignore-certificate-errors-spki-list=${pin}`]});
const report=[];mkdirSync('artifacts/fleet-portal',{recursive:true});
try {
 const page=await browser.newPage({viewport:{width:1280,height:900}});await page.goto(base);
 const devices=await page.evaluate(async()=> (await (await fetch('/api/v1/devices')).json()).devices);
 for(const device of devices.filter(d=>!d.revokedAt && (!process.argv[2] || d.name.toLowerCase().includes(process.argv[2].toLowerCase())))) {
  try {
   await page.goto(`${base}/?device=${device.id}&managed&view=console`);
   await expect(page.getByRole('heading',{name:'Console sessions',exact:true})).toBeVisible();
   await expect(page.getByRole('button',{name:'Create desktop',exact:true})).toHaveCount(0);
   const open=page.getByRole('button',{name:'Open console',exact:true});await expect(open).toBeEnabled({timeout:15000});await open.click();
   await page.waitForFunction(()=>document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames>30 && !document.querySelector('.console-placeholder'),null,{timeout:30000});
   await page.bringToFront();await page.waitForTimeout(1200);await page.locator('video').focus();await page.locator('video').click();await expect(page.locator('.console-view')).toHaveAttribute('data-input-state','controlling',{timeout:5000});
   await page.keyboard.press('Escape');
   const activities=await page.locator('video').evaluate(v=>{const b=v.getBoundingClientRect(),s=Math.min(b.width/v.videoWidth,b.height/v.videoHeight);return{x:b.x+(b.width-v.videoWidth*s)/2+30*s,y:b.y+(b.height-v.videoHeight*s)/2+15*s}});
   await page.mouse.click(activities.x,activities.y);await page.keyboard.type('calculator',{delay:90});await page.waitForTimeout(1200);await page.keyboard.press('Enter');await page.waitForTimeout(2500);
   await expect(page.locator('.console-view')).toHaveAttribute('data-input-state','controlling');
   await page.waitForTimeout(1500);
   const calculation=await page.locator('video').evaluate(v=>{const b=v.getBoundingClientRect(),s=Math.min(b.width/v.videoWidth,b.height/v.videoHeight);return{x:b.x+(b.width-v.videoWidth*s)/2+270*s,y:b.y+(b.height-v.videoHeight*s)/2+260*s}});
   await page.mouse.click(calculation.x,calculation.y);await page.keyboard.press('Escape');
   await page.keyboard.type('123',{delay:90});await page.keyboard.down('Shift');await page.keyboard.press('Equal');await page.keyboard.up('Shift');await page.keyboard.type('456',{delay:90});await page.keyboard.press('Enter');await page.waitForTimeout(1500);
   await page.waitForTimeout(500);
   await page.screenshot({path:`artifacts/fleet-portal/${device.id}.png`});
   const framesBefore=await page.locator('video').evaluate(v=>v.getVideoPlaybackQuality().totalVideoFrames);
   await page.reload();
   await page.waitForFunction(()=>document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames>30 && !document.querySelector('.console-placeholder'),null,{timeout:30000});
   report.push({name:device.name,id:device.id,video:true,inputAcquired:true,reconnect:true,framesBefore,url:page.url(),visualAcceptance:'Review screenshot for Calculator result 579'});console.log(device.name,'video + control + reconnect; visual review required');
  }catch(e){report.push({name:device.name,error:e.message.slice(0,180)});console.log(device.name,'FAILED',e.message.slice(0,150),(await page.locator('body').innerText()).slice(-600));await page.screenshot({path:`artifacts/fleet-portal/${device.id}-failure.png`}).catch(()=>{})}
  await page.goto(base);
 }
 writeFileSync('artifacts/fleet-portal/report.json',JSON.stringify(report,null,2));
 if(report.some(r=>r.error))process.exitCode=1;
}finally{await browser.close()}
