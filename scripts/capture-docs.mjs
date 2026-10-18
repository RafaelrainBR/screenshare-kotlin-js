import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { mkdir, readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// Like qa-ui.mjs, use an existing Playwright installation without a product dependency.
const require = createRequire(resolve(process.env.PLAYWRIGHT_MODULE_DIR || 'node_modules', 'package.json'));
const { chromium } = require('playwright');
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const output = resolve(root, 'docs/images');
const baseUrl = process.env.SCREENSHARE_DOCS_URL || 'http://localhost:8080';
assert(['localhost', '127.0.0.1'].includes(new URL(baseUrl).hostname), 'Use a local server for demo rooms');
const room = `docs-${Date.now().toString(36)}`;
const source = await readFile(resolve(root, 'common/src/commonMain/kotlin/screenshare/common/Packet.kt'), 'utf8');
const escape = text => text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
const preview = `<!doctype html><html><head><meta charset="utf-8"><title>Screenshare docs source</title><style>
body{margin:0;background:#10141c;color:#d1dae8;font:22px/1.4 monospace}
header{padding:24px 40px;border-bottom:1px solid #293141;color:#a9b7cc;font:18px system-ui}
header span{float:right;color:#89a3bf}pre{margin:28px 40px;white-space:pre-wrap}
</style></head><body><header>Screenshare / common / Packet.kt <span>Conteúdo de demonstração</span></header><pre>${escape(source.split('\n').slice(12, 36).join('\n'))}</pre></body></html>`;
const server = createServer((_, response) => response.end(preview));
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
let browser;
try {
  browser = await chromium.launch({ channel: 'chrome', headless: true, args: [
    '--enable-usermedia-screen-capturing',
    '--auto-select-tab-capture-source-by-title=Screenshare docs source',
    '--allow-http-screen-capture',
    '--autoplay-policy=no-user-gesture-required',
  ] });
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
  const sample = await context.newPage();
  await sample.goto(`http://127.0.0.1:${server.address().port}`);
  const errors = [];
  const newPeer = async name => {
    const page = await context.newPage();
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(baseUrl);
    await page.evaluate(() => document.fonts.ready);
    await page.locator('#username').fill(name);
    await page.locator('#roomId').fill(room);
    await page.locator('#joinBtn').click();
    await page.waitForFunction(() => !document.querySelector('#send-message-btn').disabled);
    return page;
  };
  await mkdir(output, { recursive: true });
  const join = await context.newPage();
  await join.goto(baseUrl);
  await join.evaluate(() => document.fonts.ready);
  await join.locator('.join-form').screenshot({ path: resolve(output, 'join_screen.png') });
  await join.close();
  const viewer = await newPeer('Marina');
  const sharer = await newPeer('Rafael');
  await viewer.locator('#message-input').fill('Vamos revisar o compartilhamento de tela?');
  await viewer.locator('#message-input').press('Enter');
  await sharer.locator('.chat-bubble').filter({ hasText: 'Vamos revisar' }).waitFor();
  await sharer.locator('#message-input').fill('Claro! Estou mostrando o protocolo em Kotlin.');
  await sharer.locator('#message-input').press('Enter');
  await viewer.locator('.chat-bubble').filter({ hasText: 'Claro!' }).waitFor();
  await sharer.locator('#share-picker summary').click();
  await sharer.locator('[data-menu-preset="TEXT"]').click();
  await sharer.locator('#share-picker summary').click();
  await sharer.locator('#shareScreenBtn').click();
  await viewer.waitForFunction(() => [...document.querySelectorAll('.screen-tile video')].some(video => video.videoWidth > 0), null, { timeout: 30000 });
  assert.equal(await viewer.locator('#participant-count').textContent(), '2', 'Two connected participants');
  const video = viewer.locator('.screen-tile video').first();
  await video.evaluate(video => new Promise(resolve => video.requestVideoFrameCallback(resolve)));
  // Let the screen encoder settle after its first low-resolution frames.
  await viewer.waitForTimeout(4000);
  assert(await video.evaluate(video => video.videoWidth >= 1280), 'Shared text has readable resolution');
  await viewer.screenshot({ path: resolve(output, 'room_sharing.png') });
  await viewer.locator('#sidebar-chat').screenshot({ path: resolve(output, 'sidebar.png') });
  await viewer.locator('#share-picker summary').click();
  await viewer.locator('#share-menu-main').waitFor();
  await viewer.locator('#share-picker .picker-panel').screenshot({ path: resolve(output, 'share_settings.png') });
  assert.deepEqual(errors, [], 'No client runtime errors');
  console.log('Captured join screen, two-peer WebRTC room, chat sidebar and transmission settings.');
} finally {
  await browser?.close();
  await new Promise(resolve => server.close(resolve));
}
