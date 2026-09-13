// Explicit package capture fault only. No greeter restart, login, keyboard or mouse.
import { chromium } from '@playwright/test';
import { target, browserOptions, remote, requireFlag, preflight, mainPID, evidenceDirectory, until } from './package-console-target.mjs';

requireFlag('--allow-capture-interruption');
const guardTest = process.argv.includes('--also-test-guard-recovery');
await preflight();
const browser = await chromium.launch(browserOptions);
let captureMayBeStopped = false;
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const evidence = await evidenceDirectory('capture-recovery');
  let offers = 0;
  page.on('request', request => { if (new URL(request.url()).pathname === '/api/offer') offers++; });
  await page.addInitScript(() => {
    window.packageTestPeers = [];
    const Native = window.RTCPeerConnection;
    window.RTCPeerConnection = class extends Native {
      constructor(...args) { super(...args); window.packageTestPeers.push(this); }
    };
  });
  await page.goto(target.url);
  await page.waitForFunction(() => document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames > 45, undefined, { timeout: 30000 });
  const webPID = await mainPID(target.web);
  const inputPID = await mainPID(target.input);
  const originalOffers = offers;
  const before = await page.locator('video').evaluate(v => v.getVideoPlaybackQuality().totalVideoFrames);
  await page.screenshot({ path: `${evidence}/before.png` });
  captureMayBeStopped = true; // Also restore if the stop command's SSH response is lost.
  await remote(`sudo -n systemctl stop ${target.capture}`);
  // Deliberately exceed the old five-second reconnect threshold, then await
  // explicit recovering (source inactivity detection may take ten seconds).
  await page.waitForTimeout(7000);
  await page.waitForFunction(async () => (await (await fetch('/api/status', { cache: 'no-store' })).json()).state === 'recovering', undefined, { timeout: 15000 });
  const during = await page.evaluate(() => ({
    peers: window.packageTestPeers.length, state: window.packageTestPeers[0].connectionState,
    width: document.querySelector('video').videoWidth,
    height: document.querySelector('video').videoHeight,
    frames: document.querySelector('video').getVideoPlaybackQuality().totalVideoFrames,
    overlay: document.querySelector('.console-placeholder') !== null,
    inputState: document.querySelector('[data-input-state]')?.dataset.inputState,
  }));
  if (originalOffers !== 1 || offers !== originalOffers || during.peers !== 1 || during.state !== 'connected'
      || during.width !== 1920 || during.height !== 1080 || during.overlay || during.inputState === 'controlling') {
    throw new Error('Viewer failed to preserve an inactive last frame: ' + JSON.stringify({ during, offers }));
  }
  await page.screenshot({ path: `${evidence}/waiting.png` });
  await remote(`sudo -n systemctl start ${target.capture}`);
  captureMayBeStopped = false;
  await page.waitForFunction(previous => document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames > previous + 60, during.frames, { timeout: 45000 });
  await page.waitForFunction(async () => (await (await fetch('/api/status', { cache: 'no-store' })).json()).state === 'streaming', undefined, { timeout: 10000 });
  const after = await page.evaluate(() => ({
    peers: window.packageTestPeers.length, state: window.packageTestPeers[0].connectionState,
    frames: document.querySelector('video').getVideoPlaybackQuality().totalVideoFrames,
  }));
  if (await mainPID(target.web) !== webPID || await mainPID(target.input) !== inputPID
      || after.peers !== 1 || after.state !== 'connected' || offers !== originalOffers) {
    throw new Error('Persistent viewer/input identity changed during capture recovery');
  }
  let guardRevalidated = null;
  if (guardTest) {
    const originalGuard = JSON.parse(await remote(`sudo -n cat ${target.guardFile}`));
    if (!originalGuard.valid) throw new Error('Guard was not initially valid');
    await remote(`sudo -n systemctl stop ${target.guard}`);
    await until(async () => {
      const guard = JSON.parse(await remote(`sudo -n cat ${target.guardFile}`));
      return guard.valid && guard.session !== originalGuard.session;
    });
    guardRevalidated = true;
    if (await mainPID(target.web) !== webPID || await mainPID(target.input) !== inputPID) {
      throw new Error('Guard recovery restarted persistent web/input services');
    }
    const peers = await page.evaluate(() => ({ count: window.packageTestPeers.length, state: window.packageTestPeers[0].connectionState }));
    if (peers.count !== 1 || peers.state !== 'connected' || offers !== originalOffers) throw new Error('Guard recovery disconnected viewer');
  }
  await page.screenshot({ path: `${evidence}/after.png` });
  console.log(JSON.stringify({ target: target.url, sameBridgePID: true, sameInputPID: true, samePeerConnection: true,
    offers, before, after, retainedLastFrame: true, guardRevalidated, screenshots: evidence,
    evidence: 'Inspect screenshots separately; advancing decoded frames do not prove nonblank console content.' }));
} finally {
  try { if (captureMayBeStopped) await remote(`sudo -n systemctl start ${target.capture}`); }
  finally { await browser.close(); }
}
