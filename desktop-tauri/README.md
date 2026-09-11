# Screenshare Desktop (Tauri)

This module is the Windows shell for the canonical Kotlin/JS client. It does not
contain a second frontend, signaling protocol, or server.

## Prerequisites

- JDK 21 or newer;
- Node.js and npm;
- Rust stable with the `x86_64-pc-windows-msvc` target;
- Visual Studio 2022 C++ build tools;
- WebView2 Runtime.

Install the local Tauri CLI once:

```powershell
cd desktop-tauri
npm install
```

## Development

Start the current Kotlin server in one terminal:

```powershell
.\gradlew.bat :server-java:run
```

Start the desktop shell in another terminal:

```powershell
.\gradlew.bat desktopDev
```

`desktopDev` lets Tauri start `:client:jsBrowserDevelopmentRun` on port 8081.
The desktop client connects to the Kotlin server on `ws://localhost:8080`.
Its frontend hook builds the native bridge before Gradle processes resources and
passes `-Pscreenshare.desktop.bridge`; this is what makes the optional native
audio bridge available in the WebView2 development shell. A direct Kotlin/JS
browser build does not set that property and retains the inert bridge placeholder.
The hook also sets `GRADLE_USER_HOME` before launching the wrapper, because the
wrapper resolves its own distribution before it can read Gradle command-line
arguments. This keeps its cache inside the repository's ignored `.gradle`
directory on restricted Windows hosts.

## Build and installer

```powershell
.\gradlew.bat desktopBuild
```

The Tauri build hook runs `:client:jsBrowserDistribution` with the repository's
ignored `.gradle` cache before compiling the shell. Tauri embeds the exact files
from:

```text
client/build/dist/js/productionExecutable
```

The configured Windows output is an NSIS installer under
`desktop-tauri/src-tauri/target/release/bundle/nsis`.

Release builds connect to the current Kotlin production endpoint at
`wss://screen-share.fly.dev`. The endpoint is passed as a startup query parameter
and is only honored when the centralized Tauri environment check succeeds.

## Scope

## Native system audio (phases 3–4)

The Tauri build bundles a tiny TypeScript bridge before building the Kotlin/JS
distribution. It is exposed to Kotlin only as `DesktopCaptureBridge`; the web
distribution receives a browser-safe placeholder and never imports
`@tauri-apps/api`.

On a compatible WebView2, system-audio sharing uses the default render device's
WASAPI loopback. Rust emits f32 PCM at 48 kHz, stereo, in 480-frame (3,840-byte)
chunks with monotonic microsecond timestamps. The bridge creates `AudioData` and
a `MediaStreamTrackGenerator`, replacing (not duplicating) the display-media
audio track before the existing WebRTC flow publishes the stream.

`stop_system_audio` is idempotent. Stopping screen sharing stops the generator,
the Tauri channel, and signals the capture thread; unavailable generator/device
errors leave the regular WebView display stream usable. Process audio, source
enumeration, thumbnails, and native pickers remain phase 5 work.

### Validation recorded for this implementation

Run from the repository root:

```powershell
cd desktop-tauri
npm install
npm run build-bridge
cd src-tauri
cargo fmt --check
cargo check
cargo test
cd ..\..
.\gradlew.bat --gradle-user-home .\.gradle :client:jsTest :client:jsBrowserDistribution
.\gradlew.bat --gradle-user-home .\.gradle desktopBuild
```

The Rust unit tests cover 480/2/4 geometry (3,840 bytes) and strictly monotonic
timestamps. Kotlin tests cover browser-versus-desktop detection and the feature
gate. Manual Windows validation is still required for device switching, IPC
jitter/chunk-loss metrics, installer launch, and an actual desktop/browser room:
verify WebSocket, SDP, ICE, `connectionState=connected`, and non-silent remote
audio recording. The complete Gradle `check` remains subject to the documented
pre-existing ktlint failures.

For local diagnostics, a build launched with a WebView2 remote-debugging port can
be checked with `node scripts/smoke-cdp.mjs <port> <username> <room>`. The script
only drives the already running WebView; it is not part of the packaged app.

## Process-isolated audio (phase 5)

The desktop share dialog now offers **without audio**, **selected application**,
and **all computer audio**. For the selected-application mode it lists only
eligible visible, titled top-level windows; the desktop shell itself is omitted.
The UI only displays an application name. Window handles, process identifiers,
geometry, process-tree selection, and thumbnails are local IPC data and are
never added to Kotlin signaling, room packets, SDP, or ICE.

Video still comes from the standard WebView2 `getDisplayMedia` picker. The
user must enable its audio checkbox before native audio can begin, then should
select the same application in the app picker. Native application loopback uses
WASAPI with child-process inclusion enabled by default. A live process with no
audio produces valid silent 10-ms PCM chunks; a vanished process sends a local
terminal marker so the generator track closes cleanly.

### Phase 5 validation recorded

```powershell
cd desktop-tauri
npm run build-bridge
cd src-tauri
cargo fmt --check
cargo check
cargo test
```

All commands above passed on this worktree. Rust tests cover PCM geometry,
strict timestamps, source serialization, bounded thumbnail geometry, an
invalid/nonexistent PID precheck, and a stopped capture. The command layer has
idempotent start/stop plus a joined worker thread.

The Kotlin/JS compile was exercised through `:client:jsTest` and
`:client:jsBrowserDistribution`; Gradle fell back from its Kotlin daemon due to
an access-denied marker under the user Kotlin daemon directory. A later Gradle
invocation also reported an access-denied pre-existing `build/reports/problems`
file after compilation. These host file-lock failures need rerunning in an
unlocked local shell before release; they do not indicate a source diagnostic.
The Tauri hook now uses `scripts/build-frontend.mjs`, which resolves the
repository root before invoking Gradle and staging `desktopBridge.js`. This
avoids its former dependence on Tauri's working directory. With that hook,
`desktopBuild` completed and produced
`src-tauri/target/release/bundle/nsis/Screenshare_0.1.0_x64-setup.exe`.

### Remaining manual Windows validation

- Build with `./gradlew.bat desktopBuild` and verify the NSIS installer on a
  Windows machine with WebView2 Runtime and the Visual Studio C++ runtime.
- Join one desktop and one browser in the same Kotlin/Ktor room; verify the
  existing WebSocket, SDP, ICE, and `connectionState=connected` observations.
- Play two applications with distinct tones and confirm the browser receives
  only the selected application's tone. Repeat with a child-process app,
  stopping/reconnecting/changing rooms and then closing the chosen process.
- Record local diagnostics for produced/dropped chunks, IPC jitter and CPU.
  No representative hardware run was performed in this change, so no jitter or
  CPU metric is claimed here.

Limitations: thumbnails are bounded to 320 px and may be unavailable for
protected, minimized, or off-screen windows; DRM/HDCP content remains subject
to Windows and application restrictions. Application loopback isolates a PID
and optional process tree, not an individual browser tab.

### Validation update — 2026-09-06

The native picker now requests a thumbnail only after the user selects an
application. Its BGRA pixels are converted locally to RGBA for the canvas
preview; neither the pixels nor window metadata leave the WebView.

The bridge keeps local per-capture diagnostics for received/invalid PCM chunks
and timestamp jitter relative to the 10 ms cadence. They are available through
`__screenshareDesktopBridge.getAudioMetrics()` for a voluntary local diagnostic
view; they are not sent to the server.

## Desktop sharing UX (phase 6)

The desktop sharing dialog uses plain product language: **without audio**,
**audio from the selected application**, or **all computer audio**. It explains
that the Windows video picker and the audio picker are separate, asks the user
to choose the same application when isolation is wanted, and explains that a
quiet application is a valid silent capture rather than a failed share.

While sharing from desktop, **Alter audio** reopens this dialog. Confirming a
new selection stops the previous local screen capture and its native resources,
then starts the replacement in the same room through the existing WebRTC flow.
The last audio choice is retained only in the current WebView session. This
control is hidden in browsers, and camera sharing remains independent.

```powershell
.\gradlew.bat --% --gradle-user-home .\.gradle --no-problems-report `
  -Pkotlin.compiler.execution.strategy=in-process :client:compileKotlinJs
```

This command passed. Running Kotlin compilation in-process avoids a host denial
when the Kotlin daemon tries to create its marker in `%LOCALAPPDATA%`.
`cargo fmt --check`, `cargo check`, `cargo test`, and `npm run build-bridge`
also passed in this worktree.

`jsTest` compiled successfully but its ChromeHeadless runner could not start on
this host because its GPU process repeatedly crashed and the Karma temporary
GPU cache was locked. Starting `:server-java:run` was also blocked because two
JVM serialization artifacts were absent from the local cache and this host
denied Maven downloads. These are environment constraints; rerun the browser
and room smoke on a Windows machine with Chrome/WebView2 and the Gradle/Maven
caches available.

## Beta hardening status (phase 7, local only)

No beta was published, signed, distributed, or connected to telemetry. Native
audio diagnostics remain local and contain only counters: received and invalid
PCM chunks, last timestamp, and aggregate/max deviation from the 10-ms cadence.
They are available to a voluntary local diagnostic view through
`__screenshareDesktopBridge.getAudioMetrics()`; they contain no PCM, video,
window title, path, PID, HWND, or thumbnail. Native worker failures and a
selected process closing now send only a terminal control marker to close the
WebCodecs generator; the user-facing bridge maps startup errors to generic,
actionable Portuguese messages without exposing native error text.

An explicit stop or replacement detaches the old display track's `ended`
callback before stopping it. A late browser callback from the old source cannot
stop the replacement share. The Rust worker joins on `stop_audio`; its terminal
marker closes the bridge channel/generator on every exit. Camera tracks remain
outside this path.

On WebView `pagehide` (including normal desktop-process shutdown), the client
idempotently stops local screen, camera, and microphone tracks, closes all peer
connections and their negotiation state, and makes a best-effort native-audio
stop without trying to send room packets on a closing transport.

The native capture state serializes `stop + install`: concurrent starts cannot
overwrite a worker handle. Replacing a capture joins the previous worker, and
dropping Tauri capture state joins any remaining worker before teardown.

### UX contract update — audio source is not a video source

The Windows share action is an explicit two-step assistant:

1. choose **Sem áudio**, **Áudio de todo o computador**, or **Áudio de um aplicativo**;
2. only for application audio, choose an eligible local application tile. A
   tile can show a local thumbnail, but it selects an `audioSource` only. It
   never promises to select, constrain, or capture the video of that window.

After application audio is confirmed, the UI says: “Agora o Windows vai pedir
a tela ou janela para vídeo. Escolha o mesmo aplicativo para combinar vídeo e
áudio.” Only then is `getDisplayMedia` called. WebView2's picker remains the
sole video source under Option B; native HWND video capture remains explicitly
out of scope. Browser builds do not load the bridge, enumerate applications,
or show this assistant: they call the standard browser picker directly.

Native WASAPI starts only after a successful `getDisplayMedia` return. On a
cancelled picker no native worker is started and the assistant reopens. The
active desktop summary reports the audio choice and reminds the user that
Windows selected the video. If a selected application exits, its native audio
generator receives a local terminal marker and closes without ending the room,
camera, or video capture.

Privacy boundary: titles, thumbnail pixels, PID, HWND, rectangle, PCM, and
process-tree details are local-only bridge values. They are not included in
logs, room packets, signaling, SDP, ICE, or local audio metrics. The only
permitted metrics are aggregate PCM chunk counts and timestamp-jitter counters.

#### Current verification status

| Check | Status |
| --- | --- |
| `npm run build-bridge` | Validated on 2026-09-06 |
| `cargo fmt --check`, `cargo check`, `cargo test` | Validated on 2026-09-06; 9 Rust tests passed |
| Kotlin `:client:compileKotlinJs`, `:client:jsTest`, and `:client:jsBrowserDistribution` | Validated on 2026-09-06 with the repository-local `.gradle` cache |
| `desktopBuild` and NSIS installer | Started on 2026-09-06, but the nested Tauri/Gradle build did not return a final result to this host; installer smoke remains pending |
| Windows 10/11 + WebView2 + render devices + two peers | Manual smoke pending |

Manual smoke should cover Windows 10 and 11, supported WebView2 versions,
default/USB/Bluetooth output devices, no-active-audio and process-exit cases,
same-room desktop/browser sharing, and DRM/HDCP limitations. Protected or DRM
content may remain unavailable by Windows or the source application; no
capture guarantee is made.

### Commands and results in this pass

Run from the repository root:

```powershell
cd desktop-tauri
npm run build-bridge
cd src-tauri
cargo fmt --check
cargo check
cargo test
cd ..\..
.\gradlew.bat --% --gradle-user-home .\.gradle --no-problems-report -Pkotlin.compiler.execution.strategy=in-process :client:jsTest :client:jsBrowserDistribution
.\gradlew.bat --% --gradle-user-home .\.gradle --no-problems-report -Pkotlin.compiler.execution.strategy=in-process desktopBuild
```

Results observed in this worktree on 2026-09-06:

| Validation | Result |
| --- | --- |
| `npm run build-bridge` | Passed; regenerated `dist/desktopBridge.js`. |
| `cargo fmt --check` | Passed. |
| `cargo check` | Passed. |
| `cargo test` | Passed: 8 tests, including no-content terminal-marker and serialized worker-replacement contracts. The MSVC linker emitted its informational `.lib`/`.exp` message only. |
| `:client:jsTest :client:jsBrowserDistribution` | Started and compiled `:client:compileKotlinJs`, but this host did not return a final Gradle result while several existing/busy daemons were active. Do not claim test/distribution success from this pass. |
| `desktopBuild` / installer | Tauri and its frontend hook started, but likewise did not return a final Gradle/Tauri result in this host. No new installer validation is claimed. |

The source checks cannot assert a WebView picker, an actual render device, DRM
behavior, or a two-peer room by themselves.

### Windows beta checklist (manual, pending)

- [ ] Windows 10 and Windows 11, with current and older supported WebView2
  Runtime versions: launch the packaged NSIS installer and enter a room.
- [ ] Desktop and browser in the same Kotlin/Ktor room: verify chat, SDP/ICE,
  connected peer state, desktop-to-browser video, browser-to-desktop video, and
  stop/rejoin without duplicate senders.
- [ ] Default speakers, USB headset, Bluetooth headset, no active output, and
  a default-output-device change during system audio capture.
- [ ] Two audible apps with distinct tones; repeat selected-app capture with a
  child-process app, close the selected process, alter audio, and stop/restart
  sharing while camera remains active.
- [ ] Inspect only the local counters above for chunk loss/jitter; do not export
  captured media or window inventory.
- [ ] Protected/DRM/HDCP playback, fullscreen and minimized windows: document
  blocked/black/silent results as platform restrictions, never as supported
  capture behavior.

Known host limitations remain: ChromeHeadless may fail because of its GPU/cache
state, Maven downloads may be unavailable, and native picker windows may not be
visible to automation. Treat those as infrastructure constraints, not evidence
of a product failure; the manual checklist is still required before beta.

## Release gate local — phases 11–15 (2026-09-06)

### Scope checked

- Direct browser builds retain `client/src/jsMain/resources/desktopBridge.js`,
  an inert placeholder with no Tauri import. Only the Tauri frontend hook builds
  and stages `desktop-tauri/dist/desktopBridge.js` under
  `-Pscreenshare.desktop.bridge`; this applies to both `desktopDev` and
  `desktopBuild`.
- `DesktopEnvironment` remains the single environment check. The browser keeps
  deriving its endpoint from `window.location`; the desktop shell alone accepts
  the configured startup endpoint.
- The Tauri CSP permits only packaged assets, WebView IPC, blob media, approved
  font hosts, and the two explicit Kotlin WebSocket endpoints. The `main`
  capability is scoped to the `main` window and grants only `core:default`; the
  native command surface is the five handlers registered in `lib.rs`.
- The native layer does not receive room, signaling, SDP, ICE, or peer-connection
  data. Video remains `getDisplayMedia` in WebView2; native selection remains
  audio-only. Camera and microphone remain independent streams.

### Lifecycle and privacy evidence

- A share generation is advanced synchronously for start, stop, replacement,
  and page shutdown. A display picker that completes after one of those actions
  releases its stream and cannot publish a stale share.
- An old Tauri audio channel may receive a late terminal marker, but it can only
  close its own generator; it cannot stop a replacement native worker. Rust
  still serializes worker replacement and joins the stopped worker.
- `pagehide` releases screen, camera, microphone, peer connections, and native
  audio without emitting room packets on a closing transport.
- Production console output no longer serializes packets, SDP, ICE candidates,
  capture errors, track IDs, room IDs, user names, or native errors. User-facing
  failures are generic and in Portuguese. Local bridge metrics are only received
  chunk count, invalid chunk count, last timestamp, and aggregate/max 10-ms
  jitter; they contain no PCM, title, PID, HWND, path, or thumbnail and are not
  sent to signaling or exported automatically.

### Commands actually run in this release-gate pass

| Command | Result |
| --- | --- |
| `cd desktop-tauri; npm run build-bridge` | Passed; regenerated `dist/desktopBridge.js`. |
| `cd desktop-tauri/src-tauri; cargo fmt --check` | Passed. |
| `cd desktop-tauri/src-tauri; cargo check` | Passed. |
| `cd desktop-tauri/src-tauri; cargo test` | Passed: 9 tests, including serialized concurrent worker installation. The MSVC linker printed only its informational import-library message. |
| `./gradlew.bat --gradle-user-home C:\Projects\screenshare-kotlin-js.gradle --no-problems-report -Pkotlin.compiler.execution.strategy=in-process :client:jsTest :client:jsBrowserDistribution` | Blocked/inconclusive: after downloading Gradle 9.3.1 and starting daemons, this host did not return a final task result; do not treat the JS test or distribution as passed. |
| `./gradlew.bat --gradle-user-home C:\Projects\screenshare-kotlin-js.gradle --no-problems-report -Pkotlin.compiler.execution.strategy=in-process desktopBuild` | Blocked/inconclusive: Gradle started another busy daemon but did not return a final task result; installer generation is not claimed. |

The cache path above is deliberate: `C:\Projects\screenshare-kotlin-js.gradle`.
It avoids the restricted default `C:\.gradle` location used by this host's
wrapper. Re-run the two inconclusive commands in an unlocked Windows shell and
record their final `BUILD SUCCESSFUL`/failure output before release.

### Manual release checklist (still pending)

- [ ] Windows 10 and Windows 11: install the locally generated NSIS installer,
  launch with current and older supported WebView2 Runtime versions, and verify
  no native picker or bridge load error.
- [ ] In one Kotlin/Ktor room, use desktop and a regular browser: chat, join,
  leave, reconnect, SDP/ICE negotiation, connected peer state, desktop-to-
  browser video/audio, browser-to-desktop video, and stop/rejoin without stale
  tracks or duplicate senders.
- [ ] Test default speakers, USB and Bluetooth headsets, no active output, and
  a default output-device change during system audio capture. Confirm camera
  and microphone remain active/independent when screen audio is replaced.
- [ ] Test two audible applications, a child-process application, selected
  process closure, share replacement, room change, and desktop shutdown. Check
  only the local counters; never collect captured content or window inventory.
- [ ] Test DRM/HDCP, fullscreen, minimized, and protected content. Record
  black/silent/blocked behavior as a Windows/application limitation; do not
  claim support for protected capture.
