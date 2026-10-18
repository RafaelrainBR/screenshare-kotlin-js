import test from 'node:test';
import assert from 'node:assert/strict';
import { downloadsFromManifest, renderPage } from './build-presentation.mjs';

const fixture = { version: '1.2.3', platforms: {
  'windows-x86_64': { url: 'https://github.com/RafaelrainBR/screenshare-kotlin-js/releases/download/v1.2.3/Screenshare_1.2.3_x64-setup.exe' },
  'darwin-aarch64': { url: 'https://github.com/RafaelrainBR/screenshare-kotlin-js/releases/download/v1.2.3/Screenshare-macos-arm64.tar.gz' },
} };
test('uses the signed updater manifest URLs without inventing versionless Windows filenames', () => {
  const result = downloadsFromManifest(fixture);
  assert.equal(result.windowsUrl, fixture.platforms['windows-x86_64'].url);
  assert.equal(result.macUrl, fixture.platforms['darwin-aarch64'].url);
  assert(result.checksumsUrl.endsWith(`/v${fixture.version}/SHA256SUMS`));
});
test('rejects prereleases, foreign download origins and the wrong platform package', () => {
  assert.throws(() => downloadsFromManifest({ ...fixture, version: 'native-v0.1.0' }));
  for (const url of ['https://example.com/setup.exe', fixture.platforms['darwin-aarch64'].url]) {
    const bad = structuredClone(fixture);
    bad.platforms['windows-x86_64'].url = url;
    assert.throws(() => downloadsFromManifest(bad));
  }
});
test('escapes metadata and refuses missing translations rather than publishing placeholders', () => {
  assert.equal(renderPage('<title>{{title}}</title>', { title: 'A & "B" <C>' }), '<title>A &amp; &quot;B&quot; &lt;C&gt;</title>');
  assert.throws(() => renderPage('{{missing}}', {}));
});
test('a failed manifest refresh has a reproducible official-release fallback', () => {
  const result = downloadsFromManifest({ version: null, fallback: true, windowsUrl: 'https://example.com' });
  assert.equal(result.version, '');
  assert.equal(result.windowsUrl, 'https://github.com/RafaelrainBR/screenshare-kotlin-js/releases/latest');
  assert.equal(result.macUrl, result.windowsUrl);
});
