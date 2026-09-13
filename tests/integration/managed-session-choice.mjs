import { chromium, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

const host = process.argv[2];
if (!['192.168.69.21', '192.168.69.28'].includes(host)) throw new Error('pilot host required');
const pid = () => execFileSync('ssh', [host, 'systemctl --user show dwdesktop-managed-poc -p MainPID --value'], { encoding: 'utf8' }).trim();
const browser = await chromium.launch({ executablePath: '/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  await page.goto(`http://${host}:8095/?managed`);
  if ((await page.request.get(`http://${host}:8095/api/status`)).ok()) {
    const status = await (await page.request.get(`http://${host}:8095/api/status`)).json();
    if (status.state === 'session_ended') await page.getByRole('button', { name: 'Start new desktop', exact: true }).click();
  }
  await page.waitForFunction(() => document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames > 30);
  const before = pid();
  execFileSync('ssh', [host, 'python3 -'], { input: readFileSync('tests/integration/managed-desktop-logout.py') });
  await expect(page.getByRole('button', { name: 'Start new desktop', exact: true })).toBeVisible({ timeout: 45000 });
  await expect.poll(pid).toBe('0');
  await page.waitForTimeout(4000);
  if (pid() !== '0') throw new Error('desktop recreated without consent');
  await page.getByRole('button', { name: 'Start new desktop', exact: true }).click();
  await page.waitForFunction(() => document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames > 30, null, { timeout: 45000 });
  const after = pid();
  if (after === before || after === '0') throw new Error('explicit new desktop not created');
  await page.reload();
  await page.waitForFunction(() => document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames > 30);
  if (pid() !== after) throw new Error('reconnect replaced a live desktop');
  await page.waitForTimeout(1200);
  await page.locator('video').click();
  await expect(page.locator('.console-view')).toHaveAttribute('data-input-state', 'controlling');
  await page.getByRole('button', { name: 'Release input', exact: true }).click();
  // Restore the user's requested logged-out state after validating explicit start.
  execFileSync('ssh', [host, 'python3 -'], { input: readFileSync('tests/integration/managed-desktop-logout.py') });
  await expect(page.getByRole('button', { name: 'Start new desktop', exact: true })).toBeVisible({ timeout: 45000 });
  await expect.poll(pid).toBe('0');
  console.log(JSON.stringify({ host, before, after, logoutPrompts: true, noAutomaticCreation: true, explicitCreate: true, reconnectReusesDesktop: true, input: true }));
} finally { await browser.close(); }
