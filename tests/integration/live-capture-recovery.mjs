// Fault injection limited to our minigpu capture unit; no desktop input/logout.
import { chromium } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
const exec=promisify(execFile);
const remote=async command=>(await exec('ssh',['localuser@192.168.69.21',command],{timeout:15000})).stdout.trim();
const startCapture='sudo -n systemd-run --unit=dwconsole-vkms-capture -p RuntimeDirectory=dwconsole -p RuntimeDirectoryMode=0700 -p NoNewPrivileges=yes -p ProtectSystem=strict -p ProtectHome=yes /opt/donkeywork-desktop/bin/dwconsole-daemon --socket /run/dwconsole/stream.sock --device /dev/dri/card2 --fps 30 --encoder libx264';
const browser=await chromium.launch({executablePath:'/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome',headless:true});
let stopped=false;
try {
 const page=await browser.newPage({viewport:{width:1280,height:800}});
 let offers=0;
 page.on('request',request=>{if(request.url().endsWith('/api/offer'))offers++;});
 await page.addInitScript(()=>{
   window.testPeers=[];
   const Native=window.RTCPeerConnection;
   window.RTCPeerConnection=class extends Native {
     constructor(...args){super(...args);window.testPeers.push(this);}
   };
 });
 await page.goto('http://192.168.69.21:8090');
 await page.waitForFunction(()=>document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames>45,undefined,{timeout:30000});
 const pid=await remote('sudo -n systemctl show dwconsole-vkms-web --value -p MainPID');
 const originalOffers=offers;
 const before=await page.locator('video').evaluate(v=>v.getVideoPlaybackQuality().totalVideoFrames);
 await remote('sudo -n systemctl stop dwconsole-vkms-capture');stopped=true;
 await page.waitForTimeout(7000); // Deliberately beyond the old 5s stall/reconnect threshold.
 const during=await page.evaluate(async()=>({
   peerCount:window.testPeers.length,peerState:window.testPeers[0].connectionState,
   status:(await(await fetch('/api/status')).json()).state,
   hasFrame:document.querySelector('video').videoWidth===1920,
 }));
 if(during.peerCount!==1||during.peerState!=='connected'||during.status!=='recovering'||!during.hasFrame||offers!==originalOffers)
   throw new Error('viewer disconnected during capture gap: '+JSON.stringify({during,offers}));
 await remote(startCapture);stopped=false;
 await page.waitForFunction(previous=>document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames>previous+60,before,{timeout:30000});
 await page.waitForFunction(async()=>(await(await fetch('/api/status')).json()).state==='streaming',undefined,{timeout:10000});
 const after=await page.evaluate(()=>({peers:window.testPeers.length,state:window.testPeers[0].connectionState,
   frames:document.querySelector('video').getVideoPlaybackQuality().totalVideoFrames}));
 const newPID=await remote('sudo -n systemctl show dwconsole-vkms-web --value -p MainPID');
 if(newPID!==pid||after.peers!==1||after.state!=='connected'||offers!==originalOffers)throw new Error('viewer identity changed on recovery');
 const inputPID=await remote('sudo -n systemctl show dwconsole-input-probe --value -p MainPID');
 const guardBefore=JSON.parse(await remote('sudo -n cat /run/dwconsole-guard/state.json'));
 await remote('sudo -n systemctl stop dwconsole-input-guard');
 await page.waitForTimeout(3000);
 const guardAfter=JSON.parse(await remote('sudo -n cat /run/dwconsole-guard/state.json'));
 if(!guardAfter.valid||guardAfter.session===guardBefore.session)throw new Error('guard was not revalidated');
 if(await remote('sudo -n systemctl show dwconsole-vkms-web --value -p MainPID')!==pid
    ||await remote('sudo -n systemctl show dwconsole-input-probe --value -p MainPID')!==inputPID)
   throw new Error('supervisor restarted persistent services');
 const guardPeer=await page.evaluate(()=>({count:window.testPeers.length,state:window.testPeers[0].connectionState}));
 if(guardPeer.count!==1||guardPeer.state!=='connected'||offers!==originalOffers)throw new Error('supervisor disconnected viewer');
 console.log(JSON.stringify({captureGapSeconds:7,sameBridgePID:true,samePeerConnection:true,offers,before,after,
   retainedLastFrame:during.hasFrame,guardRevalidated:true,inputPIDUnchanged:true}));
}finally{
 if(stopped)await remote(startCapture);
 await browser.close();
}
