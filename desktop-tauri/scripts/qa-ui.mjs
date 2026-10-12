import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { resolve, basename } from 'node:path';

// Use the host's existing Playwright installation; no product dependency.
const require = createRequire(resolve(process.env.PLAYWRIGHT_MODULE_DIR || 'node_modules', 'package.json'));
const { chromium } = require('playwright');
const output = resolve('artifacts/qa/tray-stabilization');
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--autoplay-policy=no-user-gesture-required', '--disable-features=LocalNetworkAccessChecks'] });
const results = [];
const errors = [];
const room = `qa-ui-${Date.now()}-${'r'.repeat(80)}`;
async function newPage() {
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, permissions: ['camera', 'microphone', 'clipboard-read', 'clipboard-write'] });
  const page = await context.newPage();
  await page.addInitScript(() => addEventListener('unhandledrejection', e => console.log('REJECTION', e.reason?.name, e.reason?.stack || String(e.reason))));
  page.on('console', m => m.text().startsWith('REJECTION') && console.log(m.text().slice(0, 1500)));
  page.on('pageerror', error => { errors.push(error.message); console.log('pageerror', error.stack); });
  await page.route('http://localhost:8080/**', async route => {
    const url = new URL(route.request().url());
    const name = url.pathname === '/' ? 'index.html' : url.pathname.slice(1);
    try {
      const source = ['index.html', 'ui.css', 'ui.js', 'desktopBridge.js'].includes(name) || /^(brand|fonts)\//.test(name) ? `client/src/jsMain/resources/${name}` : `client/build/dist/js/productionExecutable/${basename(name)}`;
      await route.fulfill({ body: await readFile(resolve(source)), contentType: name.endsWith('.js') ? 'text/javascript' : name.endsWith('.css') ? 'text/css' : name.endsWith('.html') ? 'text/html' : name.endsWith('.svg') ? 'image/svg+xml' : name.endsWith('.ttf') ? 'font/ttf' : 'application/octet-stream' });
    } catch { await route.continue(); }
  });
  await page.goto('http://localhost:8080/');
  await page.locator('#username').fill('Mesmo Nome');
  await page.locator('#roomId').fill(room);
  await page.locator('#joinBtn').click();
  await page.waitForFunction(() => !document.querySelector('#send-message-btn').disabled);
  return page;
}
async function check(name, action) {
  await action(); results.push(name); console.log(`PASS: ${name}`);
}
async function withinViewport(page, selector) {
  const bounds = await page.locator(selector).boundingBox();
  assert(bounds, `${selector} visible`);
  const viewport = page.viewportSize();
  assert(bounds.x >= -1 && bounds.x + bounds.width <= viewport.width + 1 && bounds.y >= -1 && bounds.y + bounds.height <= viewport.height + 1, `${selector} fits ${JSON.stringify(viewport)}: ${JSON.stringify(bounds)}`);
}
try {
  const a = await newPage();
  const b = await newPage();
  await check('duplicate names stay distinct and chat is delivered', async () => {
    await a.waitForFunction(() => document.querySelectorAll('.participant-tile').length === 2);
    assert.equal(await a.locator('.participant-tile[data-owner-id="self"]').count(), 1);
    await b.locator('#message-input').fill('Mensagem de outra pessoa <b>literal</b>');
    await b.locator('#send-message-btn').click();
    await a.locator('.chat-start .chat-bubble').filter({ hasText: 'Mensagem de outra pessoa' }).waitFor();
    assert.equal(await a.locator('.chat-bubble b').count(), 0);
  });
  await check('main controls and device menus fit small viewports', async () => {
    for (const [width, height] of [[1280, 800], [800, 600], [375, 667], [320, 640]]) {
      await a.setViewportSize({ width, height });
      await withinViewport(a, '#send-message-btn');
      await withinViewport(a, '#toggleSidebarBtn');
      assert(await a.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `horizontal overflow at ${width}`);
      for (const summary of await a.locator('.device-picker summary').all()) {
        await summary.click();
        await a.waitForTimeout(30);
        await withinViewport(a, '.device-picker[open] .picker-panel');
        await a.keyboard.press('Escape');
      }
      await a.screenshot({ path: `${output}/main-${width}.png` });
    }
  });
  await a.setViewportSize({ width: 1280, height: 800 });
  // Web shares straight through getDisplayMedia; the quality dialog is desktop-only.
  await check('screen share start/stop keeps keyboard focus and reaches the peer', async () => {
    await a.locator('#shareScreenBtn').focus();
    await a.keyboard.press('Enter');
    await a.waitForFunction(() => document.activeElement?.id === 'stopSharingBtn');
    await b.waitForFunction(() => [...document.querySelectorAll('.screen-tile video')].some(video => video.videoWidth > 0));
    await a.keyboard.press('Enter');
    await a.waitForFunction(() => document.activeElement?.id === 'shareScreenBtn');
    await b.waitForFunction(() => document.querySelectorAll('.screen-tile').length === 0);
  });
  await check('camera start/stop repeat and remote playback', async () => {
    for (let i = 0; i < 3; i++) {
      await a.locator('#cameraBtn').click();
      await b.waitForFunction(() => [...document.querySelectorAll('.camera-tile video')].some(video => video.videoWidth > 0));
      await a.locator('#stopCameraBtn').click();
      await b.waitForFunction(() => document.querySelectorAll('.camera-tile').length === 0);
    }
  });
  await check('tray layout fits long room and participant names', async () => {
    await a.setViewportSize({ width: 400, height: 560 });
    await a.evaluate(() => document.body.classList.add('compact-tray'));
    await withinViewport(a, '.tray-footer');
    await withinViewport(a, '#tray-screen');
    assert(await a.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    await a.screenshot({ path: `${output}/tray-400.png` });
    await a.evaluate(() => document.body.classList.remove('compact-tray'));
    await a.setViewportSize({ width: 1280, height: 800 });
  });
  await check('leaving clears incoming camera/audio and returns to join', async () => {
    await b.locator('#cameraBtn').click();
    await b.locator('#micToggle').click();
    await a.waitForFunction(() => document.querySelectorAll('.camera-tile').length > 0 && document.querySelectorAll('audio[id^="remote-audio-"]').length > 0);
    await a.locator('#app-screen [data-tray-action="leave"]').click();
    await a.locator('#join-screen').waitFor({ state: 'visible' });
    assert.equal(await a.locator('.screen-tile, audio[id^="remote-audio-"]').count(), 0);
    await b.waitForFunction(() => document.querySelector('#participant-count').textContent === '1');
  });
  await check('reload with saved name rejoins the room; after leaving it only prefills the name', async () => {
    await b.reload();
    await b.waitForFunction(() => !document.querySelector('#send-message-btn').disabled);
    assert(await b.locator('#join-screen').isHidden());
    assert(new URL(b.url()).searchParams.get('roomId'));
    await a.reload();
    await a.locator('#join-screen').waitFor({ state: 'visible' });
    await a.waitForTimeout(1000);
    assert(await a.locator('#join-screen').isVisible());
    assert.equal(await a.locator('#username').inputValue(), 'Mesmo Nome');
  });
  assert.deepEqual(errors, [], 'no unhandled page errors');
  await writeFile(`${output}/results.json`, JSON.stringify({ passed: results, errors }, null, 2));
} finally { await browser.close(); }
