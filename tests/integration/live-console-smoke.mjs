// Explicit live display-only check. Never sends remote keyboard or mouse input.
// Run: node tests/integration/live-console-smoke.mjs http://192.168.69.28:8090
import { chromium } from '@playwright/test';
import { mkdir } from 'node:fs/promises';

const url = process.argv[2];
if (!url) throw new Error('Supply the explicitly targeted console URL');
const browser = await chromium.launch({
  executablePath: process.env.DESKTOP_TEST_CHROMIUM
    ?? '/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome',
  headless: true,
});
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(url);
  await page.waitForFunction(() => {
    const video = document.querySelector('video');
    return video && video.videoWidth > 0 && video.getVideoPlaybackQuality().totalVideoFrames >= 15;
  }, undefined, { timeout: 30000 });
  const before = await page.locator('video').evaluate(v => ({
    width: v.videoWidth, height: v.videoHeight, frames: v.getVideoPlaybackQuality().totalVideoFrames,
  }));
  await page.waitForFunction(frames => document.querySelector('video')
    .getVideoPlaybackQuality().totalVideoFrames > frames + 30, before.frames, { timeout: 15000 });
  const after = await page.locator('video').evaluate(v => ({
    width: v.videoWidth, height: v.videoHeight,
    frames: v.getVideoPlaybackQuality().totalVideoFrames,
    dropped: v.getVideoPlaybackQuality().droppedVideoFrames,
    readyState: v.readyState,
  }));
  if (errors.length) throw new Error(JSON.stringify(errors));
  await mkdir('artifacts/console-web', { recursive: true, mode: 0o700 });
  await page.screenshot({ path: 'artifacts/console-web/live-view.png' });
  console.log(JSON.stringify({ url, before, after, errors,
    evidence: 'Browser decoded continuing video frames; desktop input/freshness not tested.' }));
} finally {
  await browser.close();
}
