import { cpSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFileSync } from 'node:child_process'

const desktopDir = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const repositoryDir = resolve(desktopDir, '..')
const gradlew = join(repositoryDir, process.platform === 'win32' ? 'gradlew.bat' : 'gradlew')
const gradleUserHome = join(repositoryDir, '.gradle')
const gradleArgs = [
  '--gradle-user-home', gradleUserHome,
  '--no-problems-report',
  '-Pkotlin.compiler.execution.strategy=in-process',
  '-Pscreenshare.desktop.bridge',
]
const isDev = process.argv.includes('--dev')

// Build before Gradle processes client resources so desktopDev serves the same
// native bridge as the packaged shell. The Gradle property above scopes this
// replacement to the Tauri hook; direct browser builds keep the placeholder.
execFileSync('npm', ['run', 'build-bridge'], {
  cwd: desktopDir,
  stdio: 'inherit',
  shell: process.platform === 'win32',
})

execFileSync(
  gradlew,
  [...gradleArgs, isDev ? ':client:jsBrowserDevelopmentRun' : ':client:jsBrowserDistribution'],
  {
    cwd: repositoryDir,
    stdio: 'inherit',
    shell: process.platform === 'win32',
    // The wrapper resolves its distribution before Gradle can read
    // --gradle-user-home. Set the environment too, avoiding a host-level C:\\.gradle.
    env: { ...process.env, GRADLE_USER_HOME: gradleUserHome },
  },
)

if (!isDev) {
  cpSync(
    join(desktopDir, 'dist', 'desktopBridge.js'),
    join(repositoryDir, 'client', 'build', 'dist', 'js', 'productionExecutable', 'desktopBridge.js'),
  )
}
