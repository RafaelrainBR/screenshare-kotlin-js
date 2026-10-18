import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
execFileSync(process.execPath, ['scripts/build-presentation.mjs', '--refresh'], { cwd: root, stdio: 'inherit' });
execFileSync(resolve(root, process.platform === 'win32' ? 'gradlew.bat' : 'gradlew'), ['--build-cache', '--no-problems-report', '-Pkotlin.compiler.execution.strategy=in-process', ':server-java:buildFatJar'], {
  cwd: root, stdio: 'inherit', shell: process.platform === 'win32', env: { ...process.env, GRADLE_USER_HOME: process.env.GRADLE_USER_HOME || resolve(root, '.gradle') },
});
execFileSync('fly', ['deploy', '--remote-only', '--app', 'screen-share', '--config', 'fly.toml'], { cwd: resolve(root, 'server-java'), stdio: 'inherit' });
const response = await fetch('https://screen-share.fly.dev/about', { signal: AbortSignal.timeout(30000) });
assert(response.ok, `Presentation HTTP ${response.status}`);
const expected = await readFile(resolve(root, 'server/src/jvmMain/resources/presentation/index.html'), 'utf8');
assert.equal((await response.text()).replaceAll('\r\n', '\n'), expected.replaceAll('\r\n', '\n'), 'Production serves the generated presentation');
console.log('Verified production presentation: https://screen-share.fly.dev/about');
