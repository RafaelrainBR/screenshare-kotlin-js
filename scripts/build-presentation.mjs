import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repo = 'https://github.com/RafaelrainBR/screenshare-kotlin-js';
export const escapeHTML = value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');

export function downloadsFromManifest(manifest) {
  if (manifest?.fallback === true && manifest.version === null) {
    return { version: '', windowsUrl: `${repo}/releases/latest`, macUrl: `${repo}/releases/latest`, checksumsUrl: `${repo}/releases/latest/download/SHA256SUMS` };
  }
  assert.match(manifest.version, /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/);
  const base = `${repo}/releases/download/v${manifest.version}`;
  const windowsUrl = manifest.platforms?.['windows-x86_64']?.url;
  const macUrl = manifest.platforms?.['darwin-aarch64']?.url;
  assert.equal(windowsUrl, `${base}/Screenshare_${manifest.version}_x64-setup.exe`, 'Expected the stable installer in this repository');
  assert.equal(macUrl, `${base}/Screenshare-macos-arm64.tar.gz`, 'Expected the stable Apple Silicon archive');
  return { version: manifest.version, windowsUrl, macUrl, checksumsUrl: `${base}/SHA256SUMS` };
}

export function renderPage(template, values) {
  return template.replace(/\{\{(\w+)\}\}/g, (_, key) => {
    assert.equal(typeof values[key], 'string', `Missing presentation value: ${key}`);
    return escapeHTML(values[key]);
  });
}

async function build() {
  const refresh = process.argv.includes('--refresh');
  const check = process.argv.includes('--check');
  assert(!(refresh && check), 'Refresh and check are separate operations');
  const manifestPath = resolve(root, 'site/latest.json');
  let manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  let downloads;
  if (refresh) {
    try {
      const response = await fetch(`${repo}/releases/latest/download/latest.json`, { signal: AbortSignal.timeout(20000) });
      assert(response.ok, `Manifest HTTP ${response.status}`);
      const latest = await response.json();
      downloads = downloadsFromManifest(latest);
      manifest = latest;
      await writeFile(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
    } catch {
      console.warn('Latest manifest unavailable or invalid: using the official release page as fallback.');
      manifest = { version: null, fallback: true };
      downloads = downloadsFromManifest(manifest);
      await writeFile(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
    }
  } else {
    downloads = downloadsFromManifest(manifest);
  }
  const content = JSON.parse(await readFile(resolve(root, 'site/content.json'), 'utf8'));
  const template = await readFile(resolve(root, 'site/page.html'), 'utf8');
  const out = resolve(root, 'server/src/jvmMain/resources/presentation');
  await mkdir(out, { recursive: true });
  for (const [locale, copy] of Object.entries(content)) {
    const values = { ...copy, ...downloads, canonical: `https://screen-share.fly.dev${copy.canonicalPath}` };
    if (!downloads.version) {
      values.stableLabel = locale === 'pt' ? 'Última versão estável' : 'Latest stable release';
      values.windowsAction = locale === 'pt' ? 'Ver downloads Windows' : 'View Windows downloads';
    }
    const html = renderPage(template, values);
    const path = resolve(out, locale === 'pt' ? 'index.html' : 'en.html');
    if (check) assert.equal((await readFile(path, 'utf8')).replaceAll('\r\n', '\n'), html, `Regenerate ${path}`);
    else await writeFile(path, html);
  }
  console.log(`${check ? 'Verified' : 'Generated'} PT/EN pages; downloads: ${downloads.version || 'official release fallback'}.`);
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) await build();
