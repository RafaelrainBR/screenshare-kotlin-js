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
  page.on('console', message => message.type() === 'error' && !/favicon|app1\.ico/.test(`${message.text()} ${message.location().url}`) && errors.push(`${name} console: ${message.text()} ${message.location().url}`));
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
  // Native capture must never open the WebView picker.
  await d.evaluate(() => {
    window.__qaDisplayMediaCalls = 0;
    const original = navigator.mediaDevices.getDisplayMedia.bind(navigator.mediaDevices);
    navigator.mediaDevices.getDisplayMedia = (...args) => { window.__qaDisplayMediaCalls++; return original(...args); };
  });
  // A window that plays a quiet test tone, so app audio can be measured on the receiver.
  const toneBrowser = await chromium.launch({ channel: 'chrome', headless: false, args: ['--autoplay-policy=no-user-gesture-required', '--window-size=640,360'] });
  const tone = await toneBrowser.newPage();
  await tone.setContent('<title>QA Tone</title><body style="background:#246">QA Tone</body>');
  await tone.evaluate(() => {
    const context = new AudioContext();
    const oscillator = context.createOscillator();
    const gain = context.createGain();
    gain.gain.value = 0.05;
    oscillator.connect(gain).connect(context.destination);
    oscillator.start();
  });
  const pickSource = async (title) => {
    await d.locator('#shareScreenBtn').click();
    await d.locator('#share-source-step').waitFor();
    const tile = d.locator('#share-source-grid .audio-source-tile').filter({ hasText: title }).first();
    await tile.waitFor({ timeout: 15000 });
    await tile.click();
  };
  const receivedVideo = () => w.waitForFunction(() => [...document.querySelectorAll('.screen-tile video')].some(video => video.videoWidth > 0), null, { timeout: 20000 });
  // RMS of the received screen audio over ~1.5 s; 0 means silence or no track.
  const receivedAudioLevel = () => w.evaluate(async () => {
    const video = [...document.querySelectorAll('.screen-tile video')].find(v => v.srcObject?.getAudioTracks().length);
    if (!video) return 0;
    const context = new AudioContext();
    const analyser = context.createAnalyser();
    context.createMediaStreamSource(new MediaStream(video.srcObject.getAudioTracks())).connect(analyser);
    const samples = new Float32Array(analyser.fftSize);
    let peak = 0;
    for (let i = 0; i < 15; i++) {
      await new Promise(r => setTimeout(r, 100));
      analyser.getFloatTimeDomainData(samples);
      peak = Math.max(peak, Math.sqrt(samples.reduce((sum, s) => sum + s * s, 0) / samples.length));
    }
    await context.close();
    return peak;
  });
  const stopAndCheckClean = async () => {
    await d.locator('#stopSharingBtn').click();
    await w.waitForFunction(() => document.querySelectorAll('.screen-tile').length === 0);
    await d.waitForTimeout(300); // the native stop is asynchronous
    const before =(await d.evaluate(() => window.__screenshareDesktopBridge.getVideoMetrics())).native?.ticks;
    await d.waitForTimeout(700);
    const after = (await d.evaluate(() => window.__screenshareDesktopBridge.getVideoMetrics())).native?.ticks;
    assert.equal(after, before, 'native capture stopped');
  };
  await check('share wizard lists screens and windows; cancel does not share', async () => {
    await d.locator('#shareScreenBtn').click();
    await d.locator('#share-source-step').waitFor();
    await d.waitForFunction(() => document.querySelector('#share-source-grid canvas'), null, { timeout: 15000 });
    const groups = await d.locator('#share-source-grid .source-group').allTextContents();
    assert.deepEqual(groups, ['Telas', 'Janelas']);
    assert(await d.locator('#confirm-share').isDisabled(), 'needs a source');
    await d.screenshot({ path: `${output}/share-wizard.png` });
    await d.keyboard.press('Escape');
    assert(await d.locator('#quality-modal').evaluate(el => el.classList.contains('hidden')));
    await d.waitForTimeout(500);
    assert.equal(await d.locator('#stopSharingBtn').isHidden(), true);
  });
  await check('screen without audio reaches web natively and stops cleanly', async () => {
    await pickSource('Tela');
    await d.locator('#confirm-share').click();
    await receivedVideo();
    assert.equal(await w.evaluate(() => [...document.querySelectorAll('.screen-tile video')].some(v => v.srcObject?.getAudioTracks().length)), false);
    await stopAndCheckClean();
  });
  await check('screen with computer audio delivers non-silent audio', async () => {
    await pickSource('Tela');
    await d.locator('#share-audio-on').click();
    assert.equal(await d.locator('#share-audio-on-title').textContent(), 'Áudio do computador');
    await d.locator('#confirm-share').click();
    await receivedVideo();
    const level = await receivedAudioLevel();
    console.log(`  screen audio RMS ${level.toFixed(4)}`);
    assert(level > 0.001, 'receiver hears the test tone');
    await stopAndCheckClean();
  });
  await check('window with app audio delivers video and non-silent audio', async () => {
    await pickSource('QA Tone');
    await d.locator('#share-audio-on').click();
    assert.equal(await d.locator('#share-audio-on-title').textContent(), 'Áudio do aplicativo');
    await d.locator('#confirm-share').click();
    await receivedVideo();
    const level = await receivedAudioLevel();
    console.log(`  app audio RMS ${level.toFixed(4)}`);
    assert(level > 0.001, 'receiver hears the app tone');
    await w.screenshot({ path: `${output}/web-receives-window.png` });
    await stopAndCheckClean();
  });
  await check('closing the shared window ends the share', async () => {
    await pickSource('QA Tone');
    await d.locator('#confirm-share').click();
    await receivedVideo();
    await toneBrowser.close();
    await w.waitForFunction(() => document.querySelectorAll('.screen-tile').length === 0, null, { timeout: 10000 });
    await d.locator('#shareScreenBtn').waitFor();
  });
  await check('no WebView picker was opened', async () => {
    assert.equal(await d.evaluate(() => window.__qaDisplayMediaCalls), 0);
  });
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
