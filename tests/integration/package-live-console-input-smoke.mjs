// Package minigpu greeter only: explicit reviewed click/Escape; no credentials.
import { chromium } from '@playwright/test';
import { requireFlag, preflight, greeterOnly, evidenceDirectory } from './package-console-target.mjs';
requireFlag('--allow-reviewed-greeter-input');
const url = process.argv[2];
if (url !== 'http://192.168.69.21:8090') throw new Error('This probe targets only the approved minigpu greeter');
await preflight();
await greeterOnly();
const browser = await chromium.launch({ executablePath: process.env.DESKTOP_TEST_CHROMIUM
  ?? '/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(url);
  await page.waitForFunction(() => document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames >= 30, undefined, {timeout:30000});
  const point = await page.locator('video').evaluate(video => {
    const r=video.getBoundingClientRect(), s=Math.min(r.width/1920,r.height/1080);
    return {x:r.left+(r.width-1920*s)/2+960*s,y:r.top+(r.height-1080*s)/2+490*s};
  });
  const evidence = await evidenceDirectory('greeter-input');
  await page.screenshot({path:`${evidence}/before.png`});
  await page.mouse.click(point.x,point.y); // focus/acquire only, no remote click
  await page.locator('[data-input-state="controlling"]').waitFor({timeout:10000});
  await greeterOnly(); // Recheck immediately before actual injection; guard remains authoritative.
  await page.mouse.click(point.x,point.y); // actual remote greeter selection
  await page.waitForTimeout(800);
  await page.screenshot({path:`${evidence}/click.png`});
  await page.keyboard.press('Escape');
  await page.waitForTimeout(500);
  await page.screenshot({path:`${evidence}/escape.png`});
  await page.getByRole('button',{name:'Release input',exact:true}).click();
  await page.locator('[data-input-state="idle"]').waitFor({timeout:5000});
  await page.mouse.click(point.x,point.y);
  await page.locator('[data-input-state="controlling"]').waitFor({timeout:5000});
  const secondViewer = await page.evaluate(async () => {
    const pc = new RTCPeerConnection({iceServers:[]});
    const dc = pc.createDataChannel('dwconsole.input',{ordered:true});
    pc.addTransceiver('video',{direction:'recvonly'});
    const result = new Promise((resolve,reject) => {
      const timer = setTimeout(()=>reject(new Error('second input timeout')),10000);
      dc.onopen = ()=>dc.send(JSON.stringify({type:'acquire'}));
      dc.onmessage = message=>{clearTimeout(timer);resolve(JSON.parse(message.data));};
    });
    try {
      await pc.setLocalDescription(await pc.createOffer());
      if(pc.iceGatheringState!=='complete') await new Promise(resolve=>{
        pc.onicegatheringstatechange=()=>{if(pc.iceGatheringState==='complete')resolve();};
      });
      const response=await fetch('/api/offer',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(pc.localDescription)});
      if(!response.ok)throw new Error(`second viewer negotiation ${response.status}`);
      await pc.setRemoteDescription(await response.json());
      return await result;
    }finally{pc.close();}
  });
  if(secondViewer.type!=='unavailable')throw new Error('second viewer acquired existing control');
  await page.locator('[data-input-state="controlling"]').waitFor({timeout:5000});
  // Focus loss after held Shift; helper event observer verifies actual release separately.
  await page.keyboard.down('Shift');
  await page.getByRole('button',{name:'Switch to light mode',exact:true}).focus();
  await page.locator('[data-input-state="idle"]').waitFor({timeout:5000});
  await page.keyboard.up('Shift');
  if(errors.length) throw new Error(JSON.stringify(errors));
  console.log(JSON.stringify({url,acquire:true,release:true,reacquire:true,blurRelease:true,secondViewerDenied:true,errors,screenshots:evidence,
    evidence:'Browser actions transmitted; inspect click/Escape screenshots for actual greeter response.'}));
} finally { await browser.close(); }
