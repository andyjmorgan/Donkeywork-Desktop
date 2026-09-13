import {chromium,expect} from '@playwright/test';
import {readFileSync} from 'node:fs';
import {createHash,X509Certificate} from 'node:crypto';
const base='https://192.168.10.11:30443';
const cert=new X509Certificate(readFileSync(process.env.MANAGER_TEST_SERVER_CERT));
const pin=createHash('sha256').update(cert.publicKey.export({type:'spki',format:'der'})).digest('base64');
const browser=await chromium.launch({executablePath:'/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome',headless:true,args:[`--ignore-certificate-errors-spki-list=${pin}`]});
try{
 const page=await browser.newPage();await page.goto(base);
 const name=`Delete verification ${Date.now()}`;
 await page.getByLabel('Device name').fill(name);
 await page.getByRole('button',{name:'Create device',exact:true}).click();
 const row=page.locator('article').filter({has:page.getByRole('heading',{name,exact:true})});
 await expect(row).toBeVisible();const id=await row.getAttribute('data-device-id');
 await expect(row.getByRole('button',{name:'Delete',exact:true})).toHaveCount(0);
 const denied=await page.evaluate(async id=>(await fetch(`/api/v1/devices/${id}/delete`,{method:'POST',headers:{'Content-Type':'application/json'},body:'{}'})).status,id);
 if(denied!==409)throw Error('unrevoked deletion was not rejected');
 page.once('dialog',d=>d.accept());await row.getByRole('button',{name:'Revoke',exact:true}).click();
 await expect(row.getByRole('button',{name:'Delete',exact:true})).toBeVisible();
 page.once('dialog',d=>d.dismiss());await row.getByRole('button',{name:'Delete',exact:true}).click();await expect(row).toBeVisible();
 page.once('dialog',d=>d.accept());await row.getByRole('button',{name:'Delete',exact:true}).click();await expect(row).toHaveCount(0);
 await page.reload();await expect(page.getByRole('heading',{name:'Devices',exact:true})).toBeVisible();await expect(page.locator(`[data-device-id="${id}"]`)).toHaveCount(0);
 console.log(JSON.stringify({unrevokedDenied:true,confirmationCancel:true,revokedDeleted:true,absentAfterReload:true,onlyTestRecordDeleted:id}));
}finally{await browser.close()}
