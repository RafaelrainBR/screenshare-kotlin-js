// Desktop ↔ web QA. Launch a desktop build pointed at ws://localhost:8080 with
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223 --use-fake-device-for-media-stream --use-fake-ui-for-media-stream"
// and run the signaling server, then: node desktop-tauri/scripts/qa-desktop.mjs [cdpPort]
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const require = createRequire(resolve(process.env.PLAYWRIGHT_MODULE_DIR || 'node_modules', 'package.json'));
const { chromium } = require('playwright');
const port = process.argv[2] || '9223';
const output = resolve('artifacts/qa/desktop');
await mkdir(output, { recursive: true });
const room = `qa-desktop-${Date.now()}`;
const results = [];
const errors = [];

const desktopBrowser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
const d = desktopBrowser.contexts()[0].pages().find(page => page.url().includes('tauri.localhost'));
assert(d, 'desktop page found');
const watch = (page, name) => {
  page.on('pageerror', error => errors.push(`${name}: ${error.message}`));
  page.on('console', message => message.type() === 'error' && !/favicon|app1\.ico/.test(message.text()) && errors.push(`${name} console: ${message.text()}`));
};
watch(d, 'desktop');
const webBrowser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--autoplay-policy=no-user-gesture-required'] });
const w = await (await webBrowser.newContext({ permissions: ['camera', 'microphone'] })).newPage();
watch(w, 'web');

async function join(page, name) {
  await page.locator('#username').fill(name);
  await page.locator('#roomId').fill(room);
  await page.locator('#joinBtn').click();
  await page.waitForFunction(() => !document.querySelector('#send-message-btn').disabled);
}
async function check(name, action) {
  await action(); results.push(name); console.log(`PASS: ${name}`);
}
const visibleScreens = page => page.evaluate(() => [...document.querySelectorAll('.screen-tile video')].filter(video => video.videoWidth > 0).length);

try {
  await d.reload();
  await w.goto('http://localhost:8080/');
  await check('desktop and web join the same room', async () => {
    await join(d, 'Desktop QA');
    await join(w, 'Web QA');
    await d.waitForFunction(() => document.querySelector('#participant-count').textContent === '2');
  });
  await check('chat both ways', async () => {
    await d.locator('#message-input').fill('olá do desktop');
    await d.keyboard.press('Enter');
    await w.locator('.chat-bubble').filter({ hasText: 'olá do desktop' }).waitFor();
    await w.locator('#message-input').fill('olá da web');
    await w.keyboard.press('Enter');
    await d.locator('.chat-start .chat-bubble').filter({ hasText: 'olá da web' }).waitFor();
    assert.equal(await d.locator('#message-input').inputValue(), '');
  });
  await check('share wizard offers audio modes and cancel does not share', async () => {
    await d.locator('#shareScreenBtn').click();
    await d.locator('#desktop-wizard-step-one').waitFor();
    assert.equal(await d.locator('.audio-mode-card').count(), 3);
    await d.locator('#desktop-audio-mode-process').click();
    await d.locator('#desktop-wizard-step-two').waitFor();
    await d.waitForFunction(() => document.querySelector('#desktop-audio-picker-state').textContent.length > 0);
    await d.waitForFunction(() => document.querySelector('#desktop-audio-source-grid canvas'), null, { timeout: 15000 });
    await d.screenshot({ path: `${output}/share-process.png` });
    await d.locator('#desktop-back-to-audio-modes').click();
    await d.keyboard.press('Escape');
    assert(await d.locator('#quality-modal').evaluate(el => el.classList.contains('hidden')));
    await d.waitForTimeout(500);
    assert.equal(await d.locator('#stopSharingBtn').isHidden(), true);
  });
  for (const mode of ['none', 'system']) {
    await check(`screen share with audio mode "${mode}" reaches web and stops cleanly`, async () => {
      await d.locator('#shareScreenBtn').click();
      await d.locator(`#desktop-audio-mode-${mode}`).click();
      await d.locator('#confirm-share').click();
      await w.waitForFunction(() => [...document.querySelectorAll('.screen-tile video')].some(video => video.videoWidth > 0), null, { timeout: 20000 });
      if (mode === 'system') {
        await w.waitForFunction(() => [...document.querySelectorAll('.screen-tile video')].some(video => video.srcObject?.getAudioTracks().length > 0), null, { timeout: 10000 });
      }
      await d.locator('#stopSharingBtn').click();
      await w.waitForFunction(() => document.querySelectorAll('.screen-tile').length === 0);
    });
  }
  await check('camera on/off from main window', async () => {
    await d.locator('#cameraBtn').click();
    await w.waitForFunction(() => [...document.querySelectorAll('.camera-tile video')].some(video => video.videoWidth > 0));
    await d.locator('#stopCameraBtn').click();
    await w.waitForFunction(() => document.querySelectorAll('.camera-tile').length === 0);
  });
  const cdp = await d.context().newCDPSession(d);
  await check('compact tray layout and controls', async () => {
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 430, height: 660, deviceScaleFactor: 1, mobile: false });
    await d.evaluate(() => dispatchEvent(new CustomEvent('screenshare:viewer-mode', { detail: 'compact' })));
    await d.locator('#tray-panel').waitFor();
    assert(await d.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'no horizontal overflow');
    await d.locator('#tray-camera').click();
    await w.waitForFunction(() => [...document.querySelectorAll('.camera-tile video')].some(video => video.videoWidth > 0));
    await d.locator('#tray-mic').click();
    await d.waitForFunction(() => document.querySelector('#tray-mic').getAttribute('aria-pressed') === 'true');
    await d.screenshot({ path: `${output}/tray-compact.png` });
    await d.locator('#tray-camera').click();
    await w.waitForFunction(() => document.querySelectorAll('.camera-tile').length === 0);
  });
  await check('leave from tray releases media and notifies web', async () => {
    await d.locator('#tray-panel [data-tray-action="leave"]').click();
    await w.waitForFunction(() => document.querySelector('#participant-count').textContent === '1');
    assert.equal(await d.locator('#tray-connection-status').textContent(), 'Entre em uma sala para usar a mídia.');
    assert.equal(await d.locator('audio[id^="remote-audio-"], .screen-tile, .camera-tile').count(), 0);
    await d.evaluate(() => dispatchEvent(new CustomEvent('screenshare:viewer-mode', { detail: 'room' })));
    await cdp.send('Emulation.clearDeviceMetricsOverride');
  });
  assert.deepEqual(errors, [], 'no page errors');
  await writeFile(`${output}/results.json`, JSON.stringify({ passed: results, errors }, null, 2));
} finally {
  await webBrowser.close();
  await desktopBrowser.close().catch(() => {});
}
