import { chromium, expect } from '@playwright/test';

const browser = await chromium.launch({ executablePath: '/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  await page.addInitScript(() => {
    window.pilotChannels = [];
    const create = RTCPeerConnection.prototype.createDataChannel;
    RTCPeerConnection.prototype.createDataChannel = function (...args) {
      const channel = create.apply(this, args);
      window.pilotChannels.push(channel);
      return channel;
    };
  });
  await page.goto('http://192.168.69.21:8095/?managed');
  await page.waitForFunction(() => window.pilotChannels[0]?.readyState === 'open'
    && document.querySelector('video').getVideoPlaybackQuality().totalVideoFrames > 30);
  // Fault injection only: no desktop keys, commands, or click are delivered.
  await page.evaluate(() => window.pilotChannels[0].close());
  await page.waitForFunction(() => window.pilotChannels.length > 1
    && window.pilotChannels.at(-1).readyState === 'open'
    && document.querySelector('video').getVideoPlaybackQuality().totalVideoFrames > 30,
    null, { timeout: 30000 });
  await page.waitForTimeout(1200);
  await page.locator('video').click(); // Acquisition click is deliberately swallowed.
  await expect(page.locator('.console-view')).toHaveAttribute('data-input-state', 'controlling');
  await page.getByRole('button', { name: 'Release input', exact: true }).click();
  await expect(page.locator('.console-view')).toHaveAttribute('data-input-state', 'idle');
  console.log('PASS: closed input channel automatically replaced; video resumed; control reacquired and released. No desktop input sent.');
} finally {
  await browser.close();
}
