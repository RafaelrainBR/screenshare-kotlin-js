import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { downloadsFromManifest } from './build-presentation.mjs';

const require = createRequire(resolve(process.env.PLAYWRIGHT_MODULE_DIR || 'node_modules', 'package.json'));
const { chromium } = require('playwright');
const base = process.env.SCREENSHARE_DOCS_URL || 'http://localhost:8080';
assert(['localhost', '127.0.0.1'].includes(new URL(base).hostname), 'Use a local QA server');
const expected = downloadsFromManifest(JSON.parse(await readFile('site/latest.json', 'utf8')));
const output = resolve('artifacts/qa/presence');
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const errors = [];
const results = [];
try {
  for (const [locale, path] of [['pt', '/about'], ['en', '/about/en.html']]) {
    const context = await browser.newContext({ javaScriptEnabled: false });
    const page = await context.newPage();
    page.on('pageerror', error => errors.push(error.message));
    assert.equal((await page.goto(base + path)).status(), 200);
    assert.equal(await page.locator('html').getAttribute('lang'), locale === 'pt' ? 'pt-BR' : 'en');
    assert.equal(await page.locator('[data-download=windows]').getAttribute('href'), expected.windowsUrl);
    assert.equal(await page.locator('[data-download=mac]').getAttribute('href'), expected.macUrl);
    assert.equal(await page.locator('.actions .primary').getAttribute('href'), '/');
    assert.equal(await page.locator('video').getAttribute('autoplay'), null);
    assert.equal(await page.locator('video').getAttribute('preload'), 'none');
    assert.equal(await page.locator('h1').count(), 1);
    assert(!(await page.content()).includes('{{'));
    for (const colorScheme of ['light', 'dark']) {
      await page.emulateMedia({ colorScheme });
      for (const [width, height] of [[1440, 900], [1280, 720], [768, 1024], [390, 844], [320, 640]]) {
        await page.setViewportSize({ width, height });
        assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${locale}: overflow at ${width}`);
        if (width >= 1280) {
          const button = await page.locator('.actions .primary').boundingBox();
          assert(button.y + button.height < height, `${locale}: primary action above the fold`);
        }
        assert(await page.locator('.product-shot img').evaluate(img => img.complete && img.naturalWidth > 0), `${locale}: product image loaded`);
        await page.screenshot({ path: `${output}/${locale}-${colorScheme}-${width}.png`, fullPage: true });
      }
    }
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.keyboard.press('Tab');
    assert.equal(await page.locator(':focus').getAttribute('class'), 'skip');
    await page.keyboard.press('Enter');
    assert.equal(new URL(page.url()).hash, '#main');
    await page.locator('#mac-install summary').click();
    assert(await page.locator('#mac-install code').isVisible());
    await page.keyboard.press('Tab');
    const metaImage = await page.locator('meta[property="og:image"]').getAttribute('content');
    assert(metaImage.startsWith('https://screen-share.fly.dev/about/assets/'));
    const imageResponse = await context.request.get(base + new URL(metaImage).pathname);
    assert.equal(imageResponse.status(), 200);
    assert((await imageResponse.body()).length < 1_000_000);
    assert.equal((await context.request.head(base + new URL(metaImage).pathname)).status(), 200);
    const rangeResponse = await context.request.get(`${base}/about/assets/demo-${locale}.mp4`, { headers: { Range: 'bytes=0-1023' } });
    assert.equal(rangeResponse.status(), 206, 'Media supports seeking with byte ranges');
    assert.equal((await rangeResponse.body()).length, 1024);
    for (const asset of ['/fonts/barlow-regular.ttf', '/brand/screenshare-logo-on-dark.svg', '/brand/screenshare-logo.svg', `/about/assets/captions-${locale}.vtt`, `/about/transcript-${locale}.html`]) {
      assert.equal((await context.request.get(base + asset)).status(), 200, asset);
    }
    results.push(`${locale}: content and downloads without JavaScript, five viewports in light/dark, keyboard skip link, macOS guide, metadata and assets`);
    await context.close();
  }
  const mediaContext = await browser.newContext({ viewport: { width: 1280, height: 800 }, reducedMotion: 'reduce' });
  const media = await mediaContext.newPage();
  await media.goto(base + '/about');
  await media.locator('video').evaluate(video => { video.muted = true; return video.play(); });
  await media.waitForFunction(() => document.querySelector('video').videoWidth === 1280);
  await media.locator('video').evaluate(video => { video.pause(); video.currentTime = 22; });
  await media.waitForFunction(() => Math.abs(document.querySelector('video').currentTime - 22) < 1 && document.querySelector('video').readyState >= 2);
  await media.screenshot({ path: `${output}/video-playback.png` });
  results.push('MP4 decodes and seeks; reduced motion keeps the page usable');
  await mediaContext.close();
  const room = `presence-qa-${Date.now()}`;
  const join = async name => {
    const context = await browser.newContext();
    const page = await context.newPage();
    page.on('pageerror', error => errors.push(error.message));
    assert.equal((await page.goto(base)).status(), 200);
    await page.locator('#username').fill(name);
    await page.locator('#roomId').fill(room);
    await page.locator('#joinBtn').click();
    await page.waitForFunction(() => !document.querySelector('#send-message-btn').disabled);
    return { context, page };
  };
  const a = await join('Teste A');
  const b = await join('Teste B');
  await a.page.locator('#message-input').fill('Rotas de apresentação não alteram a sala.');
  await a.page.locator('#message-input').press('Enter');
  await b.page.locator('.chat-bubble').filter({ hasText: 'Rotas de apresentação' }).waitFor();
  await a.context.close();
  await b.context.close();
  results.push('Original / HTTP app, WebSocket upgrade, two peers and chat delivery');
  assert.deepEqual(errors, []);
  await writeFile(`${output}/acceptance.json`, JSON.stringify({ testedAt: new Date().toISOString(), base, results, errors, humanParticipants: 'not tested', nativeHardware: 'not tested' }, null, 2));
  console.log(results.map(result => `PASS: ${result}`).join('\n'));
} finally { await browser.close(); }
