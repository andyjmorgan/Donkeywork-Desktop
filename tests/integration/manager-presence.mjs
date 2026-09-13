import {chromium,expect} from '@playwright/test';
import {readFileSync,mkdirSync} from 'node:fs';
import {createHash,X509Certificate} from 'node:crypto';
const cert=new X509Certificate(readFileSync('/home/localuser/.local/state/dwdesktop-manager-preview/pki/server.crt'));
const pin=createHash('sha256').update(cert.publicKey.export({type:'spki',format:'der'})).digest('base64');
const browser=await chromium.launch({executablePath:'/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome',headless:true,args:[`--ignore-certificate-errors-spki-list=${pin}`]});
try{
 const page=await browser.newPage({viewport:{width:1440,height:1100}});
 await page.goto('https://192.168.10.11:30443');
 const row=page.locator('article').filter({has:page.getByRole('heading',{name:'Easternkingdoms device service',exact:true})});
 await expect(row).toContainText('Online',{timeout:60000});
 await expect(row.locator('.manager-state')).toHaveAttribute('data-state','online');
 await expect(row.locator('.manager-state-dot')).toBeVisible();
 await expect(page.getByRole('link',{name:'Download Linux installer (amd64 + arm64)'})).toBeVisible();
 mkdirSync('artifacts/manager-proof',{recursive:true});await page.screenshot({path:'artifacts/manager-proof/attic-agent-online.png'});
 console.log('Browser verified Easternkingdoms Online and package download link.');
 const open=row.getByRole('link',{name:'Open desktops',exact:true});
 await expect(open).toHaveAttribute('href','/?device=16df48d0-3827-4dc0-963e-04a8f65335e5&managed');
 await open.click();
 await expect(page).toHaveURL('https://192.168.10.11:30443/?device=16df48d0-3827-4dc0-963e-04a8f65335e5&managed');
 await expect(page.getByRole('button',{name:'Create desktop',exact:true})).toBeEnabled({timeout:15000});
 await expect(page.getByRole('heading',{name:'Desktops',exact:true})).toBeVisible();
 console.log('Device card Open desktops navigates to its live connection chooser. No session created or changed.');
 await page.goto('https://192.168.10.11:30443');
 await expect(row.locator('.manager-state')).toHaveAttribute('data-state','online');
 const inventory=await page.evaluate(async()=>await (await fetch('/api/v1/devices')).json());
 await page.route('**/api/v1/devices',async route=>{
  await route.fulfill({json:{devices:inventory.devices.map(d=>({...d,online:false}))}});
 });
 await expect(row.locator('.manager-state')).toHaveAttribute('data-state','offline',{timeout:15000});
 await expect(row.getByRole('button',{name:'Open desktops',exact:true})).toBeDisabled();
 await page.unroute('**/api/v1/devices');
 await expect(row.locator('.manager-state')).toHaveAttribute('data-state','online',{timeout:15000});
 await page.route('**/api/v1/devices',route=>route.abort());
 await expect(row.locator('.manager-state')).toHaveAttribute('data-state','unknown',{timeout:15000});
 await expect(row.getByRole('button',{name:'Open desktops',exact:true})).toBeDisabled();
 console.log('Mocked offline and unreachable-manager states disable navigation and never retain a green Online badge.');
}finally{await browser.close()}
