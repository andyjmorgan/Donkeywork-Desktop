// No keyboard/mouse actions. Checks browser/CLI arbitration on approved minigpu.
import { chromium } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
const exec=promisify(execFile);
const browser=await chromium.launch({executablePath:'/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome',headless:true});
try {
 const page=await browser.newPage();
 await page.goto('http://192.168.69.21:8090');
 // Do not load another video viewer from App while testing two raw peers.
 await page.evaluate(()=>{document.querySelector('video')?.pause();});
 // One raw peer plus App's view-only peer fits the two-viewer limit.
 const acquired=await page.evaluate(async()=>{
   const pc=new RTCPeerConnection({iceServers:[]});
   const dc=pc.createDataChannel('dwconsole.input',{ordered:true});
   pc.addTransceiver('video',{direction:'recvonly'});
   const pending=[];
   dc.onmessage=e=>pending.shift()?.(JSON.parse(e.data));
   const request=data=>new Promise((resolve,reject)=>{
     const timer=setTimeout(()=>reject(new Error('input response timeout')),2000);
     pending.push(value=>{clearTimeout(timer);resolve(value);});dc.send(JSON.stringify(data));
   });
   const open=new Promise(resolve=>dc.onopen=resolve);
   await pc.setLocalDescription(await pc.createOffer());
   if(pc.iceGatheringState!=='complete')await new Promise(resolve=>pc.onicegatheringstatechange=()=>{if(pc.iceGatheringState==='complete')resolve();});
   const response=await fetch('/api/offer',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(pc.localDescription)});
   if(!response.ok)throw new Error(`viewer availability ${response.status}`);
   await pc.setRemoteDescription(await response.json());await open;
   const ready=await request({type:'acquire'});
   if(ready.type!=='ready'){pc.close();return false;}
   let sequence=0;
   const timer=setInterval(()=>void request({type:'event',generation:ready.generation,sequence:++sequence,event:{type:'renew'}}),250);
   window.ownershipTest={request,pc,timer};
   return true;
 });
 if(!acquired)throw new Error('Control already owned/unavailable; no takeover attempted');
 let denied=false;
 try{await exec('ssh',['localuser@192.168.69.21','sudo -n /opt/donkeywork-desktop/bin/input-cli --socket /run/dwconsole-input/input.sock reset'],{timeout:5000});}
 catch(error){if(error.code===1)denied=true;else throw error;}
 if(!denied)throw new Error('CLI unexpectedly acquired browser ownership');
 await page.evaluate(async()=>{
   const test=window.ownershipTest;clearInterval(test.timer);
   const result=await test.request({type:'release'});
   if(result.type!=='released')throw new Error('release barrier missing');
 });
 await exec('ssh',['localuser@192.168.69.21','sudo -n /opt/donkeywork-desktop/bin/input-cli --socket /run/dwconsole-input/input.sock reset'],{timeout:5000});
 const reacquired=await page.evaluate(async()=>{
   const test=window.ownershipTest;
   const ready=await test.request({type:'acquire'});
   if(ready.type!=='ready')return false;
   await test.request({type:'release'});test.pc.close();return true;
 });
 if(!reacquired)throw new Error('browser could not reacquire after CLI');
 console.log(JSON.stringify({cliDeniedWhileBrowserOwns:true,cliWorksWhileWebIdle:true,browserReacquires:true,noKeyboardOrMouseInjected:true}));
}finally{await browser.close();}
