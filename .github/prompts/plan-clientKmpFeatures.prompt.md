# Plan: 6 Features — Settings, Screen Share Config, Chat UX

Six features grouped into three natural implementation tracks that share infrastructure.

**Decisions recorded:**
- Settings → gear icon in `Toolbar` → modal dialog
- Desktop screen source → Compose dialog listing names (no thumbnails)
- Desktop screen audio → best-effort via `runCatching`, silent fallback
- Chat scroll → smart scroll (only when user is near the bottom)

---

## Track A — New shared data types (commonMain, no platform code)

**Step 1 — Create `ScreenShareConfig.kt`**
New file `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/services/ScreenShareConfig.kt` with:
- `enum class Resolution(width, height, label)` — entries: 480p, 720p, 1080p, 1440p
- `enum class FrameRate(fps, label)` — entries: 15, 24, 30, 60
- `enum class DisplaySurface` — `MONITOR`, `WINDOW`, `BROWSER_TAB` (hint to platform)
- `data class ScreenSource(id: String, title: String, isMonitor: Boolean)` — platform-sourced list for desktop picker
- `data class ScreenShareConfig(displaySurface, resolution, frameRate, captureAudio, sourceId?)` — defaults: 1080p, 30fps, no audio, no specific source

**Step 2 — Create `DeviceConfig.kt`**
New file `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/services/DeviceConfig.kt` with:
- `data class AudioDevice(id: String, label: String)`
- `data class DeviceSettings(micDeviceId: String? = null, outputDeviceId: String? = null, micVolume: Float = 1f, outputVolume: Float = 1f)`

---

## Track B — WebRtcManager interface extensions + platform implementations

**Step 3 — Extend `WebRtcManager.kt`**
Add:
- `suspend fun startScreenShare(config: ScreenShareConfig, onStreamEnd: () -> Unit): Boolean` (replaces signature-only old method; old method becomes default to avoid breaking in-flight callers)
- `suspend fun enumerateScreenSources(): List<ScreenSource>` — desktop returns real sources; wasmJs returns `emptyList()`
- `suspend fun enumerateAudioInputs(): List<AudioDevice>`
- `suspend fun enumerateAudioOutputs(): List<AudioDevice>`
- `suspend fun applyDeviceSettings(settings: DeviceSettings)` — switches mic device and applies volumes
- Keep old `startScreenShare(onStreamEnd)` as a delegation default using `ScreenShareConfig()` defaults so `SessionManager` compiles without changes until that step

**Step 4 — Update `WebRtcManagerDesktop.kt`**
- `enumerateScreenSources()`: call webrtc-java's `VideoDesktopCapturer.getSources()` (or `MediaDevices.getDisplayMedia` source list) — iterate to build `List<ScreenSource>`; filter into monitors vs windows
- `startScreenShare(config, onStreamEnd)`: pass `width`, `height`, `frameRate` constraints to `getDisplayMedia`. If `config.sourceId != null`, use `VideoDesktopCapturer` with that source and wrap in a `MediaStream`. Wrap in `runCatching` for the audio capture — if `audio = true` fails (Windows), retry with `audio = false`
- `enumerateAudioInputs()` / `enumerateAudioOutputs()`: use `NativeMediaDevices.getAudioCaptureDevices()` / `getAudioRenderDevices()` from webrtc-java; map to `AudioDevice`
- `applyDeviceSettings(settings)`: if `micDeviceId` changed → call `getUserMedia` with that device constraint, replace `localMicStream`, call `recreateAllConnections()`; map `outputVolume` (0..1) to `AudioDeviceModule` master volume (0..255)

**Step 5 — Update `WebRtcJsInterop.kt`**
Add new `@JsFun` declarations:
- `jsEnumerateDevices()` — `navigator.mediaDevices.enumerateDevices()` → promise → pushes array of `{deviceId, kind, label}` to `globalThis.__deviceQueue`
- `jsRequestDisplayMediaWithConfig(width, height, fps, audio, displaySurface?)` — replaces the current `jsRequestDisplayMedia`. Builds `getDisplayMedia` constraints from params; if `displaySurface` non-null adds it as a hint; `audio` controls whether to request system audio
- `jsRequestUserMediaWithDevice(deviceId)` — `getUserMedia({ audio: { deviceId: { exact: deviceId }, …same constraints… } })` → pushes to `__micQueue`
- `jsSetOutputVolume(stream, volume)` — creates `GainNode` on a stream or adjusts `HTMLAudioElement.volume`; note: setting the OS output volume from a web page is not possible; this will only control the gain on remote audio tracks received in `<audio>` elements

**Step 6 — Update `WebRtcManagerWasmJs.kt`**
- `enumerateScreenSources()`: return `emptyList()` — browser shows its own picker
- `startScreenShare(config, onStreamEnd)`: call `jsRequestDisplayMediaWithConfig(config.resolution.width, config.resolution.height, config.frameRate.fps, config.captureAudio, config.displaySurface?.name)` instead of current `jsRequestDisplayMedia()`
- `enumerateAudioInputs()` / `enumerateAudioOutputs()`: call `jsEnumerateDevices()`, poll `__deviceQueue`, filter by `kind == "audioinput"` / `"audiooutput"`, map to `AudioDevice`
- `applyDeviceSettings(settings)`: call `jsRequestUserMediaWithDevice(...)` if mic changed; speaker volume via element-level gain if feasible

---

## Track C — SessionManager + UI wiring

**Step 7 — Update `SessionManager.kt`**
- `suspend fun getScreenSources(): List<ScreenSource>` — delegates to `webRtcManager.enumerateScreenSources()`
- `fun startScreenShare(config: ScreenShareConfig)` — replaces old `startScreenShare()`, passes `config` through to `webRtcManager`
- `suspend fun getAudioDevices(): Pair<List<AudioDevice>, List<AudioDevice>>` — returns (inputs, outputs)
- `fun applyDeviceSettings(settings: DeviceSettings)` — launches coroutine → `webRtcManager.applyDeviceSettings(settings)`

**Step 8 — Create `ScreenShareConfigDialog.kt`**
Modal `AlertDialog` composable at `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/ui/components/ScreenShareConfigDialog.kt`. Parameters: `screenSources: List<ScreenSource>`, `onConfirm: (ScreenShareConfig) -> Unit`, `onDismiss: () -> Unit`. UI sections:
- **Source** (Desktop only — hide section if `screenSources.isEmpty()`): `LazyColumn` of source items; "Tela X" or "Janela: AppName"; radio selection. If empty, show `displaySurface` hint dropdown: Monitor / Janela / Aba do navegador
- **Resolução**: `Row` of chip/button toggles: 480p, 720p, 1080p, 1440p
- **Taxa de quadros**: independent `Row` of chips: 15, 24, 30, 60 fps
- **Áudio da tela**: `Switch` or `Checkbox` labeled "Capturar áudio da tela (exclui áudio da chamada de voz)"
- Buttons: "Cancelar" / "Compartilhar"

**Step 9 — Create `SettingsDialog.kt`**
Modal `AlertDialog` at `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/ui/components/SettingsDialog.kt`. Parameters: `audioInputs: List<AudioDevice>`, `audioOutputs: List<AudioDevice>`, `currentSettings: DeviceSettings`, `onApply: (DeviceSettings) -> Unit`, `onDismiss: () -> Unit`. UI sections:
- **Microfone**: `DropdownMenu` of `audioInputs`; `Slider` 0..1 labeled "Volume do microfone"
- **Saída de áudio**: `DropdownMenu` of `audioOutputs`; `Slider` 0..1 labeled "Volume de saída"
- Buttons: "Cancelar" / "Aplicar"
- `LaunchedEffect(Unit)` in the dialog body calls the parent to load devices when dialog opens

**Step 10 — Update `Toolbar.kt`**
- Add `onOpenSettings: () -> Unit` parameter
- Add `IconButton(onClick = onOpenSettings)` with `Icons.Default.Settings` at the trailing end of the row

**Step 11 — Update `RoomScreen.kt`**
- Add `onStartScreenShare: (ScreenShareConfig) -> Unit` replacing old `onStartScreenShare: () -> Unit`; add `getScreenSources: suspend () -> List<ScreenSource>`, `getAudioDevices: suspend () -> Pair<List<AudioDevice>, List<AudioDevice>>`, `onApplySettings: (DeviceSettings) -> Unit`
- Local dialog state: `var showScreenShareDialog by remember { mutableStateOf(false) }`, `var showSettingsDialog by remember { mutableStateOf(false) }`, `var screenSources by remember { mutableStateOf<List<ScreenSource>>(emptyList()) }`, `var audioDevices by ...`
- When `showScreenShareDialog` is true AND sources not yet loaded: `LaunchedEffect` calls `getScreenSources()` → sets `screenSources`, then renders `ScreenShareConfigDialog`
- When `showSettingsDialog` is true: similar `LaunchedEffect` for audio devices, then renders `SettingsDialog`
- Pass `onOpenSettings = { showSettingsDialog = true }` to `Toolbar`
- Pass `onStartScreenShare = { showScreenShareDialog = true }` to `Toolbar` (dialog calls `onStartScreenShare(config)`)
- Fix `isSharing` in `Toolbar`: change from `state.currentSharerSocketId != null` to `state.currentSharerSocketId == state.localSocketId` — but since `localSocketId` is always null currently, use a local `isLocallySharing` flag. Add `var isLocallySharing by remember { mutableStateOf(false) }` that is set to `true` on `onStartScreenShare` and `false` on `onStopScreenShare`

**Step 12 — Update `App.kt`**
- Wire the new `RoomScreen` parameters: `onStartScreenShare = { config -> sessionManager.startScreenShare(config) }`, `getScreenSources = { sessionManager.getScreenSources() }`, `getAudioDevices = { sessionManager.getAudioDevices() }`, `onApplySettings = { sessionManager.applyDeviceSettings(it) }`

---

## Track D — Chat UX fixes (independent, small)

**Step 13 — Update `ChatPanel.kt`**

**Smart auto-scroll fix:**
Replace the existing `LaunchedEffect(messages.size)` block with:
```kotlin
val isNearBottom by remember {
    derivedStateOf {
        val lastVisible = listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0
        lastVisible >= messages.size - 3
    }
}
LaunchedEffect(messages.size) {
    if (isNearBottom || messages.size <= 1) {
        listState.animateScrollToItem(maxOf(0, messages.lastIndex))
    }
}
```
Note: `derivedStateOf` avoids recomposition on every scroll position change while still reading layout info reactively.

**Enter key to send:**
- Add `onKeyEvent` modifier to `OutlinedTextField`: detect `Key.Enter` down without `Shift` modifier → call send and consume event; `Shift+Enter` passes through for newline
- Also add `keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send)` + `keyboardActions = KeyboardActions(onSend = { send() })` for mobile/wasmJs soft-keyboard support

---

## Verification

- `./gradlew :client-kmp:compileKotlinDesktop :client-kmp:compileKotlinWasmJs` — both targets compile
- Desktop smoke test: open settings dialog → verify device lists populate; open screen share dialog → verify sources list (monitors/windows); share a window at 720p 30fps → confirm video appears; stop share → video disappears
- wasmJs smoke test: browser screen share dialog respects displaySurface hint; audio capture prompts browser "share tab audio" when enabled
- Chat: receive a message while scrolled to bottom → auto-scrolls; scroll up then receive a message → stays in place
