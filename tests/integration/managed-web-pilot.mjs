import { chromium, expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';

const host = process.argv[2];
if (!['192.168.69.21', '192.168.69.28'].includes(host)) throw new Error('explicit pilot host required');
const output = `artifacts/managed-web-proof/${host}`;
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ executablePath: '/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  await page.goto(`http://${host}:8095/?managed`);
  const video = page.locator('video');
  await page.waitForFunction(() => {
    const v = document.querySelector('video');
    return v?.videoWidth === 1920 && v.getVideoPlaybackQuality().totalVideoFrames > 30;
  }, null, { timeout: 30000 });
  await page.screenshot({ path: `${output}/video.png` });
  async function point(x, y) {
    return video.evaluate((v, { x, y }) => {
      const b = v.getBoundingClientRect(), scale = Math.min(b.width / v.videoWidth, b.height / v.videoHeight);
      return { x: b.x + (b.width - v.videoWidth * scale) / 2 + x * scale,
               y: b.y + (b.height - v.videoHeight * scale) / 2 + y * scale };
    }, { x, y });
  }
  const terminal = await point(887, 1058);
  await page.mouse.click(terminal.x, terminal.y);
  await expect(page.locator('.console-view')).toHaveAttribute('data-input-state', 'controlling', { timeout: 10000 });
  await page.mouse.click(terminal.x, terminal.y);
  await page.waitForTimeout(1500);
  await page.keyboard.type('echo webinputok', { delay: 70 });
  await page.keyboard.press('Enter');
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${output}/typed.png` });
  await page.keyboard.type('seq 1 100', { delay: 70 });
  await page.keyboard.press('Enter');
  const middle = await point(700, 420);
  await page.mouse.move(middle.x, middle.y);
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${output}/before-wheel.png` });
  await page.mouse.wheel(0, -800);
  await page.waitForTimeout(1000);
  await expect(page.locator('.console-view')).toHaveAttribute('data-input-state', 'controlling');
  await page.screenshot({ path: `${output}/after-wheel.png` });
  await page.keyboard.down('Shift');
  await page.locator('h1').click();
  await expect(page.locator('.console-view')).toHaveAttribute('data-input-state', 'idle');
  await page.keyboard.up('Shift');
  await page.mouse.click(middle.x, middle.y);
  await expect(page.locator('.console-view')).toHaveAttribute('data-input-state', 'controlling');
  await page.keyboard.type('echo releaseok', { delay: 70 });
  await page.keyboard.press('Enter');
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${output}/released.png` });
  const stats = await video.evaluate(v => ({ width: v.videoWidth, height: v.videoHeight, frames: v.getVideoPlaybackQuality().totalVideoFrames }));
  await page.reload();
  await page.waitForFunction(() => document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames > 30, null, { timeout: 30000 });
  await page.screenshot({ path: `${output}/reconnected.png` });
  await page.getByRole('button', { name: '4K', exact: true }).click();
  await page.waitForFunction(() => {
    const v = document.querySelector('video');
    return v?.videoWidth === 3840 && v.getVideoPlaybackQuality().totalVideoFrames > 30;
  }, null, { timeout: 30000 });
  await page.getByRole('button', { name: '1080p', exact: true }).click();
  await page.waitForFunction(() => {
    const v = document.querySelector('video');
    return v?.videoWidth === 1920 && v.getVideoPlaybackQuality().totalVideoFrames > 30;
  }, null, { timeout: 30000 });
  await page.waitForTimeout(1200);
  const afterResize = await point(600, 420);
  await page.mouse.click(afterResize.x, afterResize.y);
  await expect(page.locator('.console-view')).toHaveAttribute('data-input-state', 'controlling');
  await page.screenshot({ path: `${output}/resized.png` });
  const report = { host, video: stats, inputState: 'acquired, wheel acknowledged, blur released, reacquired', reconnect: true, resize: '1080p -> 4K -> 1080p through web UI',
    renderedInput: 'screenshots require visual inspection', output };
  await writeFile(`${output}/report.json`, JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} finally { await browser.close(); }
