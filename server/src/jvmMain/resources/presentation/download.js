// The page carries the versions of its last deploy; GitHub knows the newest ones.
const repo = 'RafaelrainBR/screenshare-kotlin-js';
const mac = /mac/i.test(navigator.userAgentData?.platform || navigator.platform || '');
for (const el of document.querySelectorAll('[data-os]')) el.hidden = el.dataset.os !== (mac ? 'mac' : 'windows');

for (const button of document.querySelectorAll('[data-copy]')) {
  button.addEventListener('click', async () => {
    await navigator.clipboard.writeText(document.getElementById(button.dataset.copy).textContent);
    button.textContent = button.dataset.done;
  });
}

const newer = (a, b) => a[0] - b[0] || a[1] - b[1] || a[2] - b[2];
fetch(`https://api.github.com/repos/${repo}/releases?per_page=50`)
  .then(response => (response.ok ? response.json() : []))
  .then(releases => {
    const newest = {};
    for (const release of releases) {
      const match = !release.draft && /^(native-)?v(\d+)\.(\d+)\.(\d+)$/.exec(release.tag_name);
      if (!match || (!match[1] && release.prerelease)) continue;
      const line = match[1] ? 'native' : 'classic';
      const version = match.slice(2).map(Number);
      if (!newest[line] || newer(version, newest[line].version) > 0) newest[line] = { version, tag: release.tag_name };
    }
    for (const el of document.querySelectorAll('[data-file]')) {
      const found = newest[el.dataset.line];
      if (!found) continue;
      const url = `https://github.com/${repo}/releases/download/${found.tag}/${el.dataset.file.replace('{v}', found.version.join('.'))}`;
      if (el.dataset.template) el.textContent = el.dataset.template.replace('{url}', url);
      else el.href = url;
    }
    for (const el of document.querySelectorAll('[data-version]')) {
      if (newest[el.dataset.version]) el.textContent = newest[el.dataset.version].version.join('.');
    }
  })
  .catch(() => {});
