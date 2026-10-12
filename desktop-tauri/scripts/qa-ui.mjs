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
async function newPage({ join = true, path = '/', permissions = ['camera', 'microphone', 'clipboard-read', 'clipboard-write'] } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, permissions });
  const page = await context.newPage();
  await page.addInitScript(() => addEventListener('unhandledrejection', e => console.log('REJECTION', e.reason?.name, e.reason?.stack || String(e.reason))));
  // Test hooks: drop/close the signaling socket and count peer connections without touching product code.
  await page.addInitScript(() => {
    const NativeSocket = WebSocket;
    window.__sockets = [];
    window.WebSocket = class extends NativeSocket {
      constructor(...args) { super(...args); window.__sockets.push(this); }
      send(data) { if (window.__dropText && String(data).includes(window.__dropText)) return; super.send(data); }
    };
    const NativePeer = RTCPeerConnection;
    window.__peers = 0;
    window.RTCPeerConnection = class extends NativePeer { constructor(...args) { super(...args); window.__peers++; } };
  });
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
  await page.goto(`http://localhost:8080${path}`);
  if (!join) return page;
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
    await b.locator('#message-input').press('Enter');
    await a.locator('.chat-start .chat-bubble').filter({ hasText: 'Mensagem de outra pessoa' }).waitFor();
    assert.equal(await a.locator('.chat-bubble b').count(), 0);
  });
  await check('main controls and device menus fit small viewports', async () => {
    for (const [width, height] of [[1280, 800], [800, 600], [375, 667], [320, 640]]) {
      await a.setViewportSize({ width, height });
      if (width <= 700) await withinViewport(a, '#send-message-btn');
      else assert(await a.locator('#send-message-btn').isHidden(), `send button hidden at ${width}`);
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
  await check('fonts are local and no legacy favicon or Google Fonts request', async () => {
    assert.deepEqual(await a.evaluate(() => [document.fonts.check('400 14px Barlow'), document.fonts.check('700 14px Barlow')]), [true, true]);
    assert(await a.evaluate(async () => (await fetch(document.querySelector('link[rel=icon]').href)).ok));
    assert.equal(await a.evaluate(() => performance.getEntriesByType('resource').filter(r => /fonts\.g(oogleapis|static)|app1\.ico/.test(r.name)).length), 0);
  });
  await check('chat: Enter sends on desktop, keeps draft on failure, icon button on mobile', async () => {
    await a.locator('#message-input').fill('Enviada com Enter');
    await a.locator('#message-input').press('Enter');
    await b.locator('.chat-bubble').filter({ hasText: 'Enviada com Enter' }).waitFor();
    await a.waitForFunction(() => document.querySelector('#message-input').value === '');
    await a.evaluate(() => { window.__dropText = 'RASCUNHO-PRESO'; });
    await a.locator('#message-input').fill('RASCUNHO-PRESO');
    await a.locator('#message-input').press('Enter');
    await a.locator('#ui-notice:not(.hidden)').waitFor({ timeout: 15000 });
    assert.equal(await a.locator('#message-input').inputValue(), 'RASCUNHO-PRESO');
    await a.evaluate(() => { window.__dropText = null; });
    await a.locator('#message-input').fill('');
    await a.locator('#message-input').focus();
    await a.keyboard.press('Tab');
    assert.notEqual(await a.evaluate(() => document.activeElement?.id), 'send-message-btn');
    await a.setViewportSize({ width: 375, height: 667 });
    const send = a.locator('#send-message-btn');
    assert.equal((await send.textContent()).trim(), '');
    assert.equal(await send.getAttribute('aria-label'), 'Enviar mensagem');
    await withinViewport(a, '#send-message-btn');
    await a.locator('#message-input').fill('Enviada pelo botão');
    await send.click();
    await b.locator('.chat-bubble').filter({ hasText: 'Enviada pelo botão' }).waitFor();
    await a.setViewportSize({ width: 1280, height: 800 });
  });
  await check('connection status: hidden but announced when connected, visible alert while reconnecting', async () => {
    for (const id of ['#room-connection-status', '#tray-connection-status']) {
      assert(await a.locator(id).evaluate(el => el.classList.contains('sr-only') && el.getAttribute('role') === 'status' && el.textContent === 'Conectado'));
    }
    await a.evaluate(() => window.__sockets.at(-1).close());
    await a.waitForFunction(() => ['room-connection-status', 'tray-connection-status'].every(id => {
      const el = document.getElementById(id);
      return !el.classList.contains('sr-only') && el.classList.contains('connection-alert') && /reconectar|Conectando/.test(el.textContent);
    }));
    await a.locator('#message-input').fill('Não deve sair');
    await a.locator('#message-input').press('Enter');
    await a.screenshot({ path: `${output}/reconnecting-1280.png` });
    assert(await a.locator('#room-connection-status').isVisible());
    await a.waitForFunction(() => document.querySelector('#room-connection-status').classList.contains('sr-only'), null, { timeout: 30000 });
    assert.equal(await a.locator('#message-input').inputValue(), 'Não deve sair');
    await a.locator('#message-input').fill('');
  });
  await check('tray media keeps icons and mirrors dock state', async () => {
    for (const id of ['tray-mic', 'tray-camera', 'tray-output', 'tray-mic', 'tray-camera', 'tray-output']) {
      await a.evaluate(id => document.getElementById(id).click(), id);
      await a.waitForTimeout(400);
      const state = await a.evaluate(() => ({
        svgs: ['tray-mic', 'tray-camera', 'tray-output'].map(id => document.querySelectorAll(`#${id} svg`).length),
        text: ['tray-mic', 'tray-camera', 'tray-output'].map(id => document.getElementById(id).textContent.trim()),
        mic: [document.getElementById('tray-mic').getAttribute('aria-label'), document.getElementById('micToggle').getAttribute('aria-label'), document.getElementById('tray-mic').title],
        out: [document.getElementById('tray-output').getAttribute('aria-label'), document.getElementById('outputToggle').getAttribute('aria-label'), document.getElementById('tray-output').title],
        camOn: document.getElementById('tray-camera').getAttribute('aria-pressed') === 'true',
        dockCamOn: !document.getElementById('stopCameraBtn').classList.contains('hidden'),
        camLabel: document.getElementById('tray-camera').getAttribute('aria-label'),
      }));
      assert.deepEqual(state.svgs, [1, 1, 1]);
      assert.deepEqual(state.text, ['', '', '']);
      assert.equal(state.mic[0], state.mic[1]); assert.equal(state.mic[0], state.mic[2]);
      assert.equal(state.out[0], state.out[1]); assert.equal(state.out[0], state.out[2]);
      assert.equal(state.camOn, state.dockCamOn);
      assert.equal(state.camLabel, state.camOn ? 'Parar câmera' : 'Ligar câmera');
    }
  });
  await check('tray participant rows show state icons and speaking', async () => {
    await b.locator('#micToggle').click();
    await b.locator('#cameraBtn').click();
    const row = () => a.evaluate(() => [...document.querySelectorAll('#user-list .tray-user')].find(el => el.id !== 'user-list-self'));
    await a.waitForFunction(() => {
      const r = [...document.querySelectorAll('#user-list .tray-user')].find(el => el.id !== 'user-list-self');
      return r && /Microfone ativo/.test(r.getAttribute('aria-label')) && /Câmera ligada/.test(r.getAttribute('aria-label'));
    });
    assert(await row());
    assert(await a.evaluate(() => [...document.querySelectorAll('.tray-user-icons [role=img]')].every(el => el.getAttribute('aria-label'))));
    await a.waitForFunction(() => document.querySelector('#user-list .tray-user.speaking:not(#user-list-self)'), null, { timeout: 15000 });
    await b.locator('#micToggle').click();
    await a.waitForFunction(() => {
      const r = [...document.querySelectorAll('#user-list .tray-user')].find(el => el.id !== 'user-list-self');
      return /Mudo/.test(r.getAttribute('aria-label')) && r.querySelector('.state-mic.muted');
    });
    assert.equal(await a.locator('#user-list .tray-user:not(#user-list-self) input[type=range]').count(), 1);
    await a.setViewportSize({ width: 400, height: 560 });
    await a.evaluate(() => document.body.classList.add('compact-tray'));
    await a.screenshot({ path: `${output}/tray-icons-400.png` });
    await a.evaluate(() => document.body.classList.remove('compact-tray'));
    await a.setViewportSize({ width: 1280, height: 800 });
    await b.locator('#stopCameraBtn').click();
    await a.waitForFunction(() => document.querySelectorAll('.camera-tile').length === 0);
  });
  await check('focused tile is mirrored in the rail without new connections', async () => {
    await b.locator('#shareScreenBtn').focus();
    await b.keyboard.press('Enter');
    await b.locator('#cameraBtn').click();
    await a.waitForFunction(() => document.querySelectorAll('.screen-tile video').length === 2 && [...document.querySelectorAll('.screen-tile video')].every(v => v.videoWidth > 0));
    const peers = await a.evaluate(() => window.__peers);
    await a.locator('.screen-tile:not(.camera-tile)').click({ position: { x: 20, y: 20 } });
    await a.locator('#screen-mirror').waitFor();
    assert(await a.evaluate(() => {
      const mirror = document.querySelector('#screens-rail > #screen-mirror');
      const main = document.querySelector('#screens-main .screen-tile.focused video');
      const v = mirror.querySelector('video');
      return mirror === document.querySelector('#screens-rail').firstElementChild && v.srcObject === main.srcObject && v.muted && !mirror.classList.contains('screen-tile') && mirror.tabIndex === 0;
    }));
    await a.screenshot({ path: `${output}/mirror-1280.png` });
    await a.locator('.screens-rail .camera-tile').click({ position: { x: 20, y: 20 } });
    await a.waitForFunction(() => document.querySelector('#screen-mirror')?.dataset.tileId === document.querySelector('#screens-main .screen-tile.focused').dataset.tileId);
    assert(await a.evaluate(() => document.querySelector('#screen-mirror video').srcObject === document.querySelector('#screens-main .screen-tile.focused video').srcObject));
    await a.locator('#screen-mirror').click();
    await a.waitForFunction(() => !document.querySelector('#screen-mirror') && !document.querySelector('.screen-stage.focus-mode'));
    await a.locator('.screen-tile:not(.camera-tile)').click({ position: { x: 20, y: 20 } });
    await a.locator('#screen-mirror').focus();
    await a.keyboard.press('Enter');
    await a.waitForFunction(() => !document.querySelector('#screen-mirror'));
    await a.locator('.screen-tile:not(.camera-tile)').click({ position: { x: 20, y: 20 } });
    await a.locator('#screen-mirror').waitFor();
    assert.equal(await a.evaluate(() => window.__peers), peers);
    await b.locator('#stopSharingBtn').click();
    await a.waitForFunction(() => !document.querySelector('#screen-mirror') && document.querySelectorAll('.screen-tile:not(.camera-tile)').length === 0);
    await b.locator('#stopCameraBtn').click();
    await a.waitForFunction(() => document.querySelectorAll('.screen-tile').length === 0);
  });
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
    assert.equal(await a.locator('.screen-tile, video, #screen-mirror, audio[id^="remote-audio-"]').count(), 0);
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
  await check('join form validation, room link and web-only options', async () => {
    const j = await newPage({ join: false, path: '/?roomId=sala-link' });
    assert.equal(await j.locator('#roomId').inputValue(), 'sala-link');
    assert(await j.locator('#startup-option').isHidden());
    await j.locator('#joinBtn').click();
    assert(await j.locator('#username').evaluate(el => el.validity.valueMissing), 'empty name blocked natively');
    await j.locator('#username').fill('   ');
    await j.locator('#joinBtn').click();
    assert(await j.locator('#join-error').isVisible());
    assert.equal(await j.locator('#username').getAttribute('aria-invalid'), 'true');
    await j.screenshot({ path: `${output}/join-error.png` });
    await j.context().close();
  });
  await check('denied camera shows permission recovery and retry', async () => {
    const p = await newPage();
    // The fake-UI flag auto-grants devices, so deny the camera at the API instead.
    await p.evaluate(() => {
      const original = navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices);
      window.__restoreCamera = () => { navigator.mediaDevices.getUserMedia = original; };
      navigator.mediaDevices.getUserMedia = constraints => constraints?.video ? Promise.reject(new DOMException('denied', 'NotAllowedError')) : original(constraints);
    });
    await p.locator('#cameraBtn').click();
    await p.locator('#permission-recovery:not(.hidden)').waitFor();
    await p.screenshot({ path: `${output}/permission-recovery.png` });
    await p.evaluate(() => window.__restoreCamera());
    await p.locator('#retry-media').click();
    await p.waitForFunction(() => !document.querySelector('#stopCameraBtn').classList.contains('hidden'));
    await p.context().close();
  });
  assert.deepEqual(errors, [], 'no unhandled page errors');
  await writeFile(`${output}/results.json`, JSON.stringify({ passed: results, errors }, null, 2));
} finally { await browser.close(); }
