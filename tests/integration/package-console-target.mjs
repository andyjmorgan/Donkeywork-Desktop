// Constants and bounded helpers only. Importing this module performs no host actions.
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, mkdtemp } from 'node:fs/promises';
const exec = promisify(execFile);

export const target = Object.freeze({
  ssh: 'localuser@192.168.69.21', url: 'http://192.168.69.21:8090',
  capture: 'donkeywork-desktop-capture.service',
  web: 'donkeywork-desktop-web.service', input: 'donkeywork-desktop-input.service',
  supervisor: 'donkeywork-desktop-session-supervisor.service',
  guard: 'donkeywork-desktop-input-guard.service',
  guardFile: '/run/donkeywork-desktop-guard/state.json',
  inputCLI: '/opt/donkeywork-desktop/current/bin/input-cli',
  inputSocket: '/run/donkeywork-desktop-input/input.sock',
});
export const browserOptions = {
  executablePath: process.env.DESKTOP_TEST_CHROMIUM
    ?? '/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome',
  headless: true,
};
export async function remote(command) {
  return (await exec('ssh', ['-o', 'BatchMode=yes', '-o', 'ConnectTimeout=5', target.ssh, command],
    { timeout: 15000, maxBuffer: 65536 })).stdout.trim();
}
export function requireFlag(flag) {
  if (!process.argv.includes(flag)) throw new Error(`Explicit ${flag} required; target is only minigpu ${target.url}`);
}
export async function preflight() {
  await remote(`sudo -n systemctl is-active --quiet donkeywork-desktop.target ${target.capture} ${target.web} ${target.input} ${target.supervisor}`);
  await remote(`sudo -n test -S ${target.inputSocket} && sudo -n test -x ${target.inputCLI}`);
  const profile = await remote("sudo -n sed -n 's/^PROFILE=//p' /etc/donkeywork-desktop/console.env");
  if (profile !== 'vkms') throw new Error('Package VKMS configuration required; no legacy pilot fallback');
}
export async function greeterOnly() {
  const session = await remote('loginctl show-seat seat0 -p ActiveSession --value');
  if (!/^[a-zA-Z0-9]+$/.test(session)) throw new Error('No unambiguous seat0 session');
  if (await remote(`loginctl show-session ${session} -p Class --value`) !== 'greeter') {
    throw new Error('Greeter required; refusing input into a logged-in desktop');
  }
}
export async function mainPID(unit) {
  const pid = await remote(`sudo -n systemctl show ${unit} -p MainPID --value`);
  if (!/^[1-9][0-9]*$/.test(pid)) throw new Error(`No live process for ${unit}`);
  return pid;
}
export async function evidenceDirectory(prefix) {
  await mkdir('artifacts/package-console-tests', { recursive: true, mode: 0o700 });
  return mkdtemp(`artifacts/package-console-tests/${prefix}-`);
}
export async function until(check, timeout = 15000) {
  const deadline = Date.now() + timeout;
  let lastError;
  while (Date.now() < deadline) {
    try { const value = await check(); if (value) return value; }
    catch (error) { lastError = error; }
    await new Promise(resolve => setTimeout(resolve, 200));
  }
  throw new Error(`Condition timeout${lastError ? ': ' + lastError.message : ''}`);
}
