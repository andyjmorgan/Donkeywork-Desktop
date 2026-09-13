// Only called by easternkingdoms-input-proof.py with its independent observer.
import { chromium } from '@playwright/test';
import { createInterface } from 'node:readline';
if (!process.argv.includes('--allow-modifier-pointer-probe')) throw new Error('Explicit harmless probe flag required');
const input = createInterface({input:process.stdin});
const lines = input[Symbol.asyncIterator]();
async function observed(event) {
  process.stdout.write(JSON.stringify(event)+'\n');
  const ack = await lines.next();
  if (ack.done || ack.value !== 'ok') throw new Error('Independent observer rejected input');
}
const browser = await chromium.launch({executablePath: process.env.DESKTOP_TEST_CHROMIUM ?? '/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome',headless:true});
try {
  const page = await browser.newPage({viewport:{width:1280,height:900}});
  const errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.goto('http://192.168.69.17:8090');
  await page.waitForFunction(()=>document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames>=30,undefined,{timeout:30000});
  async function point(x,y) {
    return page.locator('video').evaluate((video,[x,y])=>{
      const r=video.getBoundingClientRect(), scale=Math.min(r.width/1920,r.height/1080);
      return {x:r.left+(r.width-1920*scale)/2+x*scale,y:r.top+(r.height-1080*scale)/2+y*scale};
    },[x,y]);
  }
  const center=await point(960,540);
  await page.mouse.click(center.x,center.y); // First click acquires only; never forwarded.
  await page.locator('[data-input-state="controlling"]').waitFor({timeout:10000});
  await observed({type:'acquired'});
  for(const [x,y] of [[200,200],[960,540],[1720,880]]) {
    const p=await point(x,y);
    await page.mouse.move(p.x,p.y);
    await page.waitForTimeout(100);
    await observed({type:'move',x,y});
  }
  await page.keyboard.down('Shift');
  let shiftDeadline;
  try { await Promise.race([observed({type:'shiftDown'}),new Promise((_,reject)=>{
    shiftDeadline=setTimeout(()=>reject(new Error('Shift observation deadline exceeded')),1000);
  })]); }
  finally { clearTimeout(shiftDeadline); await page.keyboard.up('Shift'); }
  await observed({type:'shift'});
  await page.getByRole('button',{name:'Release input',exact:true}).click();
  await page.locator('[data-input-state="idle"]').waitFor({timeout:5000});
  if(errors.length)throw new Error(JSON.stringify(errors));
  await observed({type:'released',errors});
} finally {input.close();process.stdin.pause();await browser.close();}
