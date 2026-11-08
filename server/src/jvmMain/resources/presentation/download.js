// The page carries the versions of its last deploy; GitHub knows the newest ones.
const repo = 'RafaelrainBR/screenshare';
const mac = /mac/i.test(navigator.userAgentData?.platform || navigator.platform || '');
for (const el of document.querySelectorAll('[data-os]')) el.hidden = el.dataset.os !== (mac ? 'mac' : 'windows');

for (const button of document.querySelectorAll('[data-copy]')) {
  button.addEventListener('click', async () => {
    await navigator.clipboard.writeText(document.getElementById(button.dataset.copy).textContent);
    button.textContent = button.dataset.done;
  });
}

const newer = (a, b) => a[0] - b[0] || a[1] - b[1] || a[2] - b[2] || (a[3] === b[3] ? 0 : a[3] > b[3] ? 1 : -1);
fetch(`https://api.github.com/repos/${repo}/releases?per_page=50`)
  .then(response => (response.ok ? response.json() : []))
  .then(releases => {
    const newest = {};
    for (const release of releases) {
      const match = !release.draft && /^(native-|android-)?v(\d+)\.(\d+)\.(\d+)(?:-rc\.(\d+))?$/.exec(release.tag_name);
      const line = match && (match[1] ? match[1].slice(0, -1) : 'classic');
      // Only Android offers test versions; a final version beats its release candidates.
      if (!match || (line !== 'android' && (release.prerelease || match[5]))) continue;
      const version = [...match.slice(2, 5).map(Number), match[5] ? Number(match[5]) : Infinity];
      if (!newest[line] || newer(version, newest[line].version) > 0) newest[line] = { version, tag: release.tag_name, name: release.tag_name.replace(/^\w+-v|^v/, '') };
    }
    for (const el of document.querySelectorAll('[data-file]')) {
      const found = newest[el.dataset.line];
      if (!found) continue;
      const url = `https://github.com/${repo}/releases/download/${found.tag}/${el.dataset.file.replace('{v}', found.name)}`;
      if (el.dataset.template) el.textContent = el.dataset.template.replace('{url}', url);
      else el.href = url;
    }
    for (const el of document.querySelectorAll('[data-version]')) {
      if (newest[el.dataset.version]) el.textContent = newest[el.dataset.version].name;
    }
  })
  .catch(() => {});
