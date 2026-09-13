import { chromium, expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';

const host = process.argv[2];
if (!['192.168.69.28', '192.168.69.21', '192.168.69.17'].includes(host)) throw new Error('pilot required');
const base = `http://${host}:8095`;
const output = `artifacts/managed-broker-proof/${host}`;
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ executablePath: '/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const inventory = async () => (await (await page.request.get(base + '/api/desktops')).json()).desktops;
  await page.goto(base + '/?managed');
  await expect(page.getByRole('button', { name: 'Create desktop', exact: true })).toBeEnabled();
  for (const environment of ['gnome', 'xfce']) {
    if (!(await inventory()).some(d => d.environment === environment)) {
      await page.getByLabel('Desktop environment', { exact: true }).selectOption(environment);
      await page.getByRole('button', { name: 'Create desktop', exact: true }).click();
    }
    await expect.poll(async () => (await inventory()).some(d => d.environment === environment && d.state === 'ready'), { timeout: 45000 }).toBe(true);
  }
  const desktops = await inventory();
  const gnome = desktops.find(d => d.environment === 'gnome'), xfce = desktops.find(d => d.environment === 'xfce');
  if (gnome.id === xfce.id) throw new Error('desktop IDs overlap');
  await page.screenshot({ path: `${output}/inventory.png` });
  const pid = id => host === '192.168.69.17' && execFileSync('hostname', {encoding:'utf8'}).trim() === 'easternkingdoms'
    ? execFileSync('systemctl', ['--user','show',`dwdesktop-session-${id}`,'-p','MainPID','--value'],{encoding:'utf8'}).trim()
    : execFileSync('ssh', [host, `systemctl --user show dwdesktop-session-${id} -p MainPID --value`], { encoding: 'utf8' }).trim();
  const gnomePid = pid(gnome.id);
  for (const desktop of [gnome, xfce]) {
    await page.goto(base + '/?managed');
    await page.locator(`[data-desktop-id="${desktop.id}"]`).getByRole('button', { name: 'Reconnect', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames > 30, null, { timeout: 30000 });
    await page.waitForTimeout(1200);
    const video = page.locator('video');
    if (desktop.environment === 'gnome') {
      await video.click();
      await expect(page.locator('.console-view')).toHaveAttribute('data-input-state', 'controlling');
      const activities = await video.evaluate(v => {
        const b = v.getBoundingClientRect(), s = Math.min(b.width/v.videoWidth, b.height/v.videoHeight);
        return { x: b.x + (b.width-v.videoWidth*s)/2+30*s, y: b.y+(b.height-v.videoHeight*s)/2+15*s };
      });
      await page.mouse.click(activities.x, activities.y);
      await page.waitForTimeout(600);
      await page.keyboard.type(host === '192.168.69.17' ? 'Text Editor' : 'terminal', { delay: 100 });
      await page.waitForTimeout(1200);
      await page.keyboard.press('Enter');
    } else {
      const point = await video.evaluate(v => {
        const b = v.getBoundingClientRect(), s = Math.min(b.width/v.videoWidth, b.height/v.videoHeight);
        return { x: b.x + (b.width-v.videoWidth*s)/2+887*s, y: b.y+(b.height-v.videoHeight*s)/2+1058*s };
      });
      await page.mouse.click(point.x, point.y);
      await expect(page.locator('.console-view')).toHaveAttribute('data-input-state', 'controlling');
      await page.mouse.click(point.x, point.y);
    }
    await page.waitForTimeout(1500);
    await page.keyboard.type(`echo ${desktop.environment}webok`, { delay: 75 });
    await page.keyboard.press('Enter');
    await page.waitForTimeout(600);
    await page.screenshot({ path: `${output}/${desktop.environment}-input.png` });
    await page.getByRole('button', { name: '4K', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('video')?.videoWidth === 3840, null, { timeout: 30000 });
    await expect(page.getByRole('button', { name: '1080p', exact: true })).toBeEnabled({ timeout: 15000 });
    await page.getByRole('button', { name: '1080p', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('video')?.videoWidth === 1920, null, { timeout: 30000 });
  }
  await page.goto(base + '/?managed');
  page.once('dialog', dialog => dialog.accept());
  await page.locator(`[data-desktop-id="${xfce.id}"]`).getByRole('button', { name: 'End desktop', exact: true }).click();
  await expect.poll(async () => (await inventory()).some(d => d.id === xfce.id), { timeout: 15000 }).toBe(false);
  if (pid(gnome.id) !== gnomePid) throw new Error('ending Xfce replaced GNOME');
  await page.locator(`[data-desktop-id="${gnome.id}"]`).getByRole('button', { name: 'Reconnect', exact: true }).click();
  await page.waitForFunction(() => document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames > 30);
  await page.screenshot({ path: `${output}/gnome-survives.png` });
  const report = { host, gnome: gnome.id, xfce: xfce.id, concurrent: true, environments: ['gnome', 'xfce'],
    resize: 'both 1080p -> 4K -> 1080p', endingOnePreservesOther: true, gnomePid, output };
  await writeFile(`${output}/report.json`, JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} finally { await browser.close(); }
