// Package browser/CLI arbitration. No key/button/move commands or desktop login.
import { chromium } from '@playwright/test';
import { target, browserOptions, remote, requireFlag, preflight } from './package-console-target.mjs';

requireFlag('--allow-control-acquisition');
await preflight();
const cliReset = `sudo -n ${target.inputCLI} --socket ${target.inputSocket} reset`;
// Establish that the CLI, its permissions and target guard work before testing denial.
await remote(cliReset);
const browser = await chromium.launch(browserOptions);
try {
  const page = await browser.newPage();
  await page.goto(target.url);
  await page.waitForFunction(() => document.querySelector('video')?.getVideoPlaybackQuality().totalVideoFrames > 15, undefined, { timeout: 30000 });
  await page.evaluate(async () => {
    const pc = new RTCPeerConnection({ iceServers: [] });
    const dc = pc.createDataChannel('dwconsole.input', { ordered: true });
    pc.addTransceiver('video', { direction: 'recvonly' });
    let pending = null;
    let tail = Promise.resolve();
    dc.onmessage = event => {
      const response = JSON.parse(event.data);
      if (pending) { const resolve = pending; pending = null; resolve(response); }
    };
    const request = data => {
      const result = tail.then(() => new Promise((resolve, reject) => {
        const timeout = setTimeout(() => { pending = null; reject(new Error('input reply timeout')); }, 1000);
        pending = value => { clearTimeout(timeout); resolve(value); };
        dc.send(JSON.stringify(data));
      }));
      tail = result;
      return result;
    };
    const waitFor = condition => new Promise((resolve, reject) => {
      const end = Date.now() + 10000;
      const timer = setInterval(() => {
        if (condition()) { clearInterval(timer); resolve(); }
        else if (Date.now() >= end) { clearInterval(timer); reject(new Error('peer negotiation timeout')); }
      }, 20);
    });
    try {
      await pc.setLocalDescription(await pc.createOffer());
      await waitFor(() => pc.iceGatheringState === 'complete');
      const response = await fetch('/api/offer', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(pc.localDescription) });
      if (!response.ok) throw new Error(`peer capacity/negotiation ${response.status}`);
      await pc.setRemoteDescription(await response.json());
      await waitFor(() => dc.readyState === 'open');
      const ready = await request({ type: 'acquire' });
      if (ready.type !== 'ready') throw new Error('Control unavailable; no takeover attempted');
      let sequence = 0;
      let timer;
      let renewing = true;
      let failure = null;
      async function renew() {
        try {
          const sent = ++sequence;
          const ack = await request({ type: 'event', generation: ready.generation, sequence: sent, event: { type: 'renew' } });
          if (ack.type !== 'ack' || ack.sequence !== sent || ack.accepted !== true) throw new Error('renew rejected');
          if (renewing) timer = setTimeout(renew, 250);
        } catch (error) { failure = error.message; renewing = false; pc.close(); }
      }
      timer = setTimeout(renew, 250);
      window.packageOwnership = {
        async release() {
          renewing = false; clearTimeout(timer);
          if (failure) throw new Error(failure);
          const result = await request({ type: 'release' });
          if (result.type !== 'released') throw new Error('release barrier missing');
        },
        async reacquire() {
          const result = await request({ type: 'acquire' });
          if (result.type !== 'ready') throw new Error('browser reacquire failed');
          const released = await request({ type: 'release' });
          if (released.type !== 'released') throw new Error('final release barrier missing');
          pc.close();
        },
        healthy() { return failure === null && pc.connectionState === 'connected'; },
      };
    } catch (error) { pc.close(); throw error; }
  });
  let denied = false;
  try { await remote(cliReset); }
  catch (error) {
    if (error.code === 1 && String(error.stderr).includes('input rejected')) denied = true;
    else throw error;
  }
  if (!denied || !await page.evaluate(() => window.packageOwnership.healthy())) {
    throw new Error('CLI denial or ongoing browser ownership was not established');
  }
  await page.evaluate(() => window.packageOwnership.release());
  await remote(cliReset);
  await page.evaluate(() => window.packageOwnership.reacquire());
  console.log(JSON.stringify({ target: target.url, cliWorksInitially: true, cliDeniedWhileBrowserOwns: true,
    cliWorksWhileWebIdle: true, browserReacquires: true, noKeyboardOrMouseActions: true }));
} finally { await browser.close(); }
