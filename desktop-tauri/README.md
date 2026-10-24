# Screenshare Desktop (Tauri)

The Windows/macOS shell embeds the canonical Kotlin/JS client. Browser and desktop use the same signaling server, protocol and rooms. Native capture lives in [`desktop-capture/`](../desktop-capture); the TypeScript bridge connects its tracks to the existing WebRTC flow.

For downloads, permissions and troubleshooting, see the [usage guide](../docs/usage.md). This page covers development and releases.

## Prerequisites

- JDK 21, Node.js/npm and Rust stable.
- Windows: MSVC target, Visual Studio C++ build tools/Windows SDK and WebView2 Runtime.
- macOS: Apple Silicon Mac with Xcode and command-line tools. The release workflow uses macOS 26 for the Swift bridge SDK; application-audio capture requires macOS 14.2 or later at runtime.

Install the pinned JavaScript tooling once from this directory:

```bash
npm ci
```

## Development

Run the Kotlin server from the repository root in one terminal:

```powershell
.\gradlew.bat :server-java:run
```

Start the desktop shell from the repository root in another:

```powershell
.\gradlew.bat desktopDev
```

On macOS, use `./gradlew` instead. `desktopDev` starts the Kotlin/JS development server on port 8081; the shell connects to signaling at `ws://localhost:8080`.

The [frontend hook](scripts/build-frontend.mjs) builds the TypeScript bridge before Gradle processes resources, then passes `-Pscreenshare.desktop.bridge`. It sets `GRADLE_USER_HOME` to the repository-local ignored `.gradle` directory before starting the wrapper. Direct browser builds omit the property and retain the inert bridge placeholder.

## Build

From the repository root:

```powershell
.\gradlew.bat desktopBuild
```

Or, from `desktop-tauri/`, use the cross-platform CLI:

```bash
npm run tauri -- build
```

The hook builds `client/build/dist/js/productionExecutable`; Tauri embeds those assets. Bundles are written below `desktop-tauri/src-tauri/target/release/bundle/`. The release workflow explicitly builds Windows NSIS/MSI plus a portable EXE, and an Apple Silicon macOS app archive. Release builds connect to `wss://screen-share.fly.dev` via the startup URL in [`tauri.conf.json`](src-tauri/tauri.conf.json).

For an instance you host, adjust both that startup URL and the `connect-src` allowlist. The browser resolves signaling from its page host; a packaged app has no equivalent deployed page host.

## Capture and lifecycle

- Windows video uses DXGI Desktop Duplication for screens and PrintWindow for windows. Window capture can have a lower practical frame rate than full-screen capture.
- Windows audio uses WASAPI loopback for computer audio or a selected process tree. Computer audio leaves out call apps (Discord, Teams, Zoom, WhatsApp…) and Screenshare itself; see [platform limits](../docs/usage.md#share-a-screen-and-audio).
- macOS uses ScreenCaptureKit for video/system audio and Core Audio taps for application audio, including descendant audio processes.
- The bridge produces media tracks for Kotlin/JS; it is not another frontend or signaling service. Source IDs, window metadata, thumbnails and PCM stay on the local capture path.
- Stop, replacement, room exit and shutdown release the corresponding media resources. Camera and microphone are independent of screen sharing.

## Validation

From `desktop-tauri/`:

```bash
npm run build-bridge
cd src-tauri
cargo fmt --check
cargo test --release
```

Run Kotlin checks from the root as described in [development](../docs/development.md#tests-and-checks). Bridge builds regenerate tracked `dist/` assets; include those when changing bridge source.

Real-device checks remain necessary for changes to capture or packaging:

- Install/launch the bundle; join a room with a browser peer and verify chat, video and audible remote audio in both directions.
- Exercise screen/window selection, system/application audio, permissions and source closure.
- Stop/restart sharing, switch rooms/devices, and keep camera/microphone active independently.
- Test tray controls, closing/reopening the main window, startup at login and quitting.
- Check updates outside a room. The portable EXE does not update itself. On macOS, verify Screen Recording permission recovery after an update.

Automated tests do not establish physical-device capture, DRM behavior, installer launch or network quality. Record the platform and results when reporting manual validation.

## Publish a release

The [Release workflow](../.github/workflows/release.yml) prepares a versioned source commit/tag, runs Kotlin/native tests, builds both platforms, uploads assets/checksums and publishes only after both builds succeed.

1. Merge the intended source changes into `master`.
2. Open **Actions → Release → Run workflow**, select `master` and enter `vX.Y.Z`.
3. Review build results and perform the relevant real-device smoke checks.

From the CLI:

```bash
gh workflow run release.yml --ref master -f tag=vX.Y.Z
```

No manual version edits or tag creation are needed. The workflow versions the desktop npm manifest/lock, Cargo manifest/lock and Tauri config. The release tag points to that commit without moving `master`. Notes in `docs/releases/vX.Y.Z.md` take precedence over generated notes. Manually pushed `v*` tags also trigger the workflow.

The updater requires the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as used by the workflow. These sign updater artifacts; Windows executables remain unsigned and macOS uses ad hoc code signing without notarization. Updater signatures, `latest.json` and `SHA256SUMS` accompany the release assets.

Re-run failed jobs to retry. If the workflow needed a fix, dispatch it again from `master` with the existing version tag: it still builds the tag's original source. Use a new version to include new app source. A changed tag stops publication, partial uploads remain a draft, and an already published release is never overwritten.

The workflow caches Gradle, npm and Cargo work per platform. See its source for current runner/SDK versions and exact asset packaging.

After publishing a stable release, refresh and deploy the public presentation so its static download links match the new updater manifest. Follow [web publication](../docs/presence/web-publication.md); the website assets are separate from the Tauri frontend bundle.
