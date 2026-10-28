import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repo = 'https://github.com/RafaelrainBR/screenshare-kotlin-js';
const semver = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
export const escapeHTML = value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');

/** Links to the release native-vX.Y.Z (the tags keep the prefix of the betas). */
export function nativeDownloads(version) {
  if (version === null) {
    // shortcut: without a known version every link opens the release list; download.js fixes them once GitHub answers.
    const list = `${repo}/releases`;
    return { nativeVersion: '', nativeSetupUrl: list, nativePortableUrl: list, nativeMacUrl: list, nativeMacScriptUrl: list, nativeChecksumsUrl: list };
  }
  assert.match(version, semver);
  const base = `${repo}/releases/download/native-v${version}`;
  return {
    nativeVersion: version,
    nativeSetupUrl: `${base}/Screenshare-native_${version}_x64-setup.exe`,
    nativePortableUrl: `${base}/Screenshare-native_${version}_x64-portable.exe`,
    nativeMacUrl: `${base}/Screenshare-native-macos-arm64.tar.gz`,
    nativeMacScriptUrl: `${base}/install-macos-native.sh`,
    nativeChecksumsUrl: `${base}/SHA256SUMS`,
  };
}

/** Newest non-draft native-vX.Y.Z tag in a GitHub releases listing, or null. */
export function newestNative(releases) {
  const versions = releases.filter(release => !release.draft)
    .map(release => /^native-v(.+)$/.exec(release.tag_name)?.[1])
    .filter(version => semver.test(version ?? ''));
  const key = version => version.split('.').map(Number);
  versions.sort((a, b) => { const [x, y] = [key(a), key(b)]; return y[0] - x[0] || y[1] - x[1] || y[2] - x[2]; });
  return versions[0] ?? null;
}

export function renderPage(template, values) {
  return template.replace(/\{\{(\w+)\}\}/g, (_, key) => {
    assert.equal(typeof values[key], 'string', `Missing presentation value: ${key}`);
    return escapeHTML(values[key]);
  });
}

async function refreshNative(nativePath) {
  let native;
  try {
    const response = await fetch('https://api.github.com/repos/RafaelrainBR/screenshare-kotlin-js/releases?per_page=50', { signal: AbortSignal.timeout(20000) });
    assert(response.ok, `Releases HTTP ${response.status}`);
    native = { version: newestNative(await response.json()) };
  } catch {
    console.warn('Native releases unavailable: linking the release list as fallback.');
    native = { version: null };
  }
  await writeFile(nativePath, JSON.stringify(native, null, 2) + '\n');
}

async function build() {
  const refresh = process.argv.includes('--refresh');
  const check = process.argv.includes('--check');
  assert(!(refresh && check), 'Refresh and check are separate operations');
  const nativePath = resolve(root, 'site/native.json');
  if (refresh) await refreshNative(nativePath);
  const native = nativeDownloads(JSON.parse(await readFile(nativePath, 'utf8')).version);
  const content = JSON.parse(await readFile(resolve(root, 'site/content.json'), 'utf8'));
  const pages = [
    ['page.html', copy => copy.canonicalPath, locale => (locale === 'pt' ? 'index.html' : 'en.html')],
    ['download.html', copy => copy.dlCanonicalPath, locale => (locale === 'pt' ? 'download.html' : 'download-en.html')],
  ];
  const out = resolve(root, 'server/src/jvmMain/resources/presentation');
  await mkdir(out, { recursive: true });
  for (const [templateName, canonicalPath, fileName] of pages) {
    const template = await readFile(resolve(root, 'site', templateName), 'utf8');
    for (const [locale, copy] of Object.entries(content)) {
      const values = { ...copy, ...native, canonical: `https://screen-share.fly.dev${canonicalPath(copy)}` };
      const html = renderPage(template, values);
      const path = resolve(out, fileName(locale));
      if (check) assert.equal((await readFile(path, 'utf8')).replaceAll('\r\n', '\n'), html, `Regenerate ${path}`);
      else await writeFile(path, html);
    }
  }
  console.log(`${check ? 'Verified' : 'Generated'} PT/EN pages; native ${native.nativeVersion || 'release list fallback'}.`);
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) await build();
