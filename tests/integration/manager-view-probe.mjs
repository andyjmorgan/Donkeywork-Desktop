import {chromium} from '@playwright/test';
import {readFileSync} from 'node:fs';
import {createHash,X509Certificate} from 'node:crypto';
const cert=new X509Certificate(readFileSync('/home/localuser/.local/state/dwdesktop-manager-preview/pki/server.crt'));
const pin=createHash('sha256').update(cert.publicKey.export({type:'spki',format:'der'})).digest('base64');
const b=await chromium.launch({executablePath:'/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome',args:[`--ignore-certificate-errors-spki-list=${pin}`]});
try{
 const p=await b.newPage({viewport:{width:1280,height:900}});
 p.on('response',r=>{if(r.url().includes('/offer'))console.log('Offer HTTP',r.status())});
 await p.goto('https://192.168.10.11:30443/?device=16df48d0-3827-4dc0-963e-04a8f65335e5&managed&desktop=22267530ec9b4269b87fb2c4871cab79');
 try{await p.waitForFunction(()=>document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames>30,null,{timeout:20000});console.log('Relayed video decoded')}
 catch(e){console.log(await p.locator('body').innerText());throw e}
 finally{await p.screenshot({path:'artifacts/manager-session-proof/view-probe.png'})}
}finally{await b.close()}
