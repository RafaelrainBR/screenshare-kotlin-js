import { createRequire } from 'node:module';
import { readFile, mkdir } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { escapeHTML } from './build-presentation.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const require = createRequire(resolve(process.env.PLAYWRIGHT_MODULE_DIR || 'node_modules', 'package.json'));
const { chromium } = require('playwright');
const data = async (path, mime) => `data:${mime};base64,${(await readFile(resolve(root, path))).toString('base64')}`;
const content = JSON.parse(await readFile(resolve(root, 'site/content.json'), 'utf8'));
const font = await data('client/src/jsMain/resources/fonts/barlow-bold.ttf', 'font/ttf');
const logo = await data('client/src/jsMain/resources/brand/screenshare-logo-on-dark.svg', 'image/svg+xml');
const room = await data('docs/images/room_sharing.png', 'image/png');
const output = resolve(root, 'server/src/jvmMain/resources/presentation/assets');
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ channel: 'chrome', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 640 }, deviceScaleFactor: 1 });
  for (const locale of ['pt', 'en']) {
    const copy = content[locale];
    await page.setContent(`<!doctype html><meta charset="utf-8"><style>@font-face{font-family:Barlow;src:url('${font}');font-weight:700}*{box-sizing:border-box}body{margin:0;background:#101318;color:#eef2fa;font:700 24px Barlow}header{position:absolute;left:64px;top:40px}header img{width:112px;height:87px}h1{position:absolute;left:64px;top:197px;width:520px;margin:0;font-size:58px;line-height:1.08;letter-spacing:-1.8px}figure{position:absolute;left:640px;top:136px;width:600px;margin:0;border:1px solid #354151;border-radius:10px;overflow:hidden}figure img{display:block;width:100%}.copy{position:absolute;left:64px;top:387px;width:455px;color:#aab6c8;line-height:1.4;font-size:25px}.url{position:absolute;left:64px;bottom:44px;color:#8eb4fa;font-size:24px}</style><header><img src="${logo}" alt="Screenshare"></header><h1>${escapeHTML(copy.headline)}</h1><figure><img src="${room}" alt="${escapeHTML(copy.imageAlt)}"></figure><p class="copy">${escapeHTML(copy.intro)}</p><p class="url">screen-share.fly.dev/about</p>`);
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({ path: resolve(output, `social-${locale}.png`) });
  }
} finally { await browser.close(); }
console.log('Rendered 1280 × 640 social images with the existing logo and real product screenshot.');
