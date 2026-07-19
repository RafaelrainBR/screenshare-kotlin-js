# Codex Guide

This is the canonical guide for Codex working in this repository.

Other assistant files may exist for compatibility, but Codex should treat this file as the source of truth. Keep it concrete, current, and focused on helping Codex choose the right files, preserve project invariants, and verify changes.

## Codex Operating Loop

For every non-trivial task:

1. Read the relevant source before proposing architecture or editing.
2. Check current worktree state with `git status --short` and do not overwrite unrelated user changes.
3. Decide whether the task targets `client`, `client-kmp`, `server`, `common`, or multiple modules.
4. Make narrow edits that follow existing local patterns.
5. Run focused verification commands when feasible.
6. Report what changed, what was verified, and which checks were skipped.

Prefer implementation over long proposals when the user asks for a concrete change. Ask only when the missing decision cannot be inferred from code and a wrong assumption would be costly.

For documentation or planning tasks, still inspect the current repo first. This project is in transition, and stale assumptions are easy.

## Project Snapshot

Screenshare is a real-time screen-sharing, voice-chat, and text-chat app built with Kotlin Multiplatform.

The current production-style web app is a browser Kotlin/JS client served by a Ktor/JVM signaling server. A newer `client-kmp` module is also present and is the direction for a future Compose Multiplatform client that targets Desktop JVM and wasmJs.

Core architecture:

- Signaling, presence, and text chat use a Ktor WebSocket endpoint at `/`.
- Screen sharing and voice media use WebRTC peer-to-peer mesh connections.
- The server does not relay media and is not an SFU/TURN/media server.
- Rooms are in-memory only. There is no database, authentication, or durable history.
- All client/server messages are defined in `common/src/commonMain/kotlin/screenshare/common/Packet.kt`.

## Repository Map

- `common/`: shared KMP protocol and DTOs. Keep platform-neutral.
- `server/`: Ktor WebSocket signaling server, room lifecycle, chat history, user presence, SDP/ICE relay.
- `server-java/`: JVM application wrapper, fat JAR, Docker, Fly.io deployment.
- `client/`: legacy Kotlin/JS browser app using DOM, DaisyUI/Tailwind CDN, browser WebRTC APIs.
- `client-kmp/`: newer Compose Multiplatform client for Desktop JVM and wasmJs.
- `libs/webrtc-kmp/`: included build/fork dependency for native WebRTC. Do not edit unless the task is explicitly about that fork.
- `docs/ai-analysis/`: older detailed AI analysis. Useful for legacy `client` patterns, but may lag behind `client-kmp`.
- `docs/images/`: screenshots used by README/docs.

## Which Client To Touch

Prefer `client-kmp` for new app architecture, shared UI/business logic, Desktop work, wasmJs work, or future rewrites.

Use `client` when the task explicitly targets the current legacy browser app, DOM/DaisyUI behavior, or the static assets bundled by `:server-java:run`.

When a feature affects the protocol or server behavior, update both clients when practical. If only one client is updated, call that out clearly.

For future reorganization or rewrite planning:

- Treat `client-kmp` as the likely destination for shared client concepts.
- Treat `common/Packet.kt` as the stable compatibility contract unless the rewrite explicitly includes a protocol migration.
- Avoid deleting legacy `client` behavior until there is a working replacement path or the user explicitly asks for removal.
- Capture migration decisions in docs before making broad structural changes.

## Protocol Rules

`Packet.kt` is the wire contract. Treat it as the single source of truth.

When adding or changing protocol messages:

1. Add or update the nested `Packet` data class.
2. Use `@Serializable` and a kebab-case `@SerialName`.
3. Keep short serialized field names where the protocol already uses them:
   - `rid`: room ID
   - `sid`: socket/sender ID
   - `tid`: target ID
   - `msg`: chat message/content
   - `ice`: ICE candidate payload
4. Update `Packet.getSide()` with `CLIENT` or `SERVER`.
5. Update server handling in `Room.consumePacket()` or connection setup in `Application.kt`.
6. Update client handling:
   - legacy client: `client/src/jsMain/kotlin/services/SessionHandler.kt`
   - KMP client: `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/services/PacketHandler.kt`
7. Add or update serialization tests in `common/src/commonTest/kotlin/screenshare/common/MessageSpec.kt`.

Do not change the SDP map shape casually. The clients expect `description["type"]` and `description["sdp"]`.

Do not change the ICE candidate payload casually. It is currently serialized as a JSON string for browser/Desktop interop, and the desktop client parses that shape manually.

## Server Patterns

Key files:

- `server/src/jvmMain/kotlin/screenshare/server/Application.kt`
- `server/src/jvmMain/kotlin/screenshare/server/Room.kt`
- `server/src/jvmMain/kotlin/screenshare/server/RoomUser.kt`

Rules:

- `JoinRoom` is handled in the WebSocket endpoint before a `RoomUser` exists.
- After join, route packets to `Room.consumePacket()`.
- Ignore server-side packets sent by clients by checking `packet.getSide() != CLIENT`.
- Create rooms lazily and remove them when empty.
- Replay chat history to a newly joined user before broadcasting join state.
- After membership changes, broadcast both the specific event and a fresh `UserList`.
- Keep room state in memory unless the task explicitly introduces persistence.
- Avoid blocking work inside WebSocket receive/broadcast paths.

## Legacy Browser Client Patterns

Key files:

- `client/src/jsMain/kotlin/Main.kt`
- `client/src/jsMain/kotlin/services/Session.kt`
- `client/src/jsMain/kotlin/services/WebsocketService.kt`
- `client/src/jsMain/kotlin/services/PeerConnections.kt`
- `client/src/jsMain/kotlin/ui/Elements.kt`
- `client/src/jsMain/kotlin/ui/InterfaceHandlers.kt`
- `client/src/jsMain/kotlin/ui/InterfaceMutations.kt`
- `client/src/jsMain/kotlin/decorators/*`

Rules:

- `Session` owns local room state and service instances.
- `WebsocketService` has one private `sendPacket(Packet)` path; public methods wrap packet creation.
- `PeerConnections` keeps one `RTCPeerConnectionDecorator` per remote socket ID.
- Always call `addTracksIfNotPresent()` before creating offers or renegotiating.
- Keep browser `dynamic` WebRTC access inside `decorators/`.
- Keep DOM lookups centralized in `Elements.kt`.
- Keep DOM mutations in `InterfaceMutations` or `ui/mutations/*`.
- Keep UI event registration in `InterfaceHandlers.kt`.
- UI text should remain Brazilian Portuguese. Existing system-message usernames use `Sistema`; own messages use `Você`.

## KMP Client Patterns

Key files:

- `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/state/AppState.kt`
- `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/services/SessionManager.kt`
- `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/services/WebsocketService.kt`
- `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/services/PacketHandler.kt`
- `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/services/WebRtcManager.kt`
- `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/ui/App.kt`
- `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/ui/screens/*`
- `client-kmp/src/commonMain/kotlin/screenshare/clientkmp/ui/components/*`
- `client-kmp/src/desktopMain/kotlin/screenshare/clientkmp/services/WebRtcManagerDesktop.kt`
- `client-kmp/src/wasmJsMain/kotlin/screenshare/clientkmp/services/WebRtcManagerWasmJs.kt`

Rules:

- `AppState` is the shared state holder. Update state immutably with `copy(...)`.
- `SessionManager` orchestrates user actions and connects UI callbacks to WebSocket/WebRTC services.
- `PacketHandler` dispatches incoming protocol messages and updates `AppState`.
- `WebsocketService` owns the Ktor client WebSocket session and packet send helpers.
- `WebRtcManager` is an `expect`/`actual` boundary. Keep platform-specific WebRTC details out of common UI/state code.
- `currentVideoTrack` is intentionally `StateFlow<Any?>` because the concrete track type is platform-specific.
- Compose UI should be mostly stateless: read state, render, and call callbacks.
- `WebRtcManagerDesktop` uses `webrtc-kmp` and native `webrtc-java` runtime libraries.
- `WebRtcManagerWasmJs` uses JS interop queues/polling for browser APIs because normal JS Promise awaiting is constrained in Kotlin/WasmJs.
- Keep wasmJs JS interop inside wasmJs source-set services/interop files.

## Build And Run Commands

On Windows PowerShell, use `.\gradlew.bat`. On Unix-like shells, use `./gradlew`.

Useful commands:

- Run bundled legacy web app + signaling server: `.\gradlew.bat :server-java:run`
- Build legacy browser bundle: `.\gradlew.bat :client:jsBrowserDistribution`
- Build server fat JAR: `.\gradlew.bat :server-java:buildFatJar`
- Run KMP desktop client: `.\gradlew.bat :client-kmp:run`
- Run KMP wasmJs dev server: `.\gradlew.bat :client-kmp:wasmJsBrowserDevelopmentRun --continuous`
- Run all checks: `.\gradlew.bat check`
- Run formatting/lint check: `.\gradlew.bat ktlintCheck`

The server listens on port `8080` by default. The KMP wasmJs webpack dev server normally serves from port `8081` and proxies to the backend.

## Gradle And Dependency Rules

- Keep versions in `libs.versions.toml`.
- Use version catalog aliases in Gradle files.
- The root build applies ktlint to subprojects.
- `settings.gradle.kts` includes `libs/webrtc-kmp` with dependency substitution for `com.shepeliev:webrtc-kmp`.
- Code in `common/commonMain` must not use JVM-only, JS-only, or browser-only APIs.
- Prefer KMP source-set boundaries over runtime platform checks.

## Feature Workflows

New client-to-server action:

1. Add a `Packet` client message.
2. Add a `WebsocketService` send helper.
3. Add the user action in `Session` or `SessionManager`.
4. Wire UI event/callback.
5. Handle it in `Room.consumePacket()`.
6. Test packet serialization and any state handling.

New server-to-client notification:

1. Add a `Packet` server message.
2. Broadcast/send it from `Room`.
3. Handle it in each relevant client packet handler.
4. Update UI state/mutations.
5. Test serialization and packet handling.

New KMP UI component:

1. Keep state in `AppState` or a focused state data class.
2. Pass state and callbacks into the composable.
3. Put platform-specific rendering behind `expect`/`actual` only when needed.
4. Add focused tests for pure state logic.

New media/WebRTC capability:

1. Decide whether it affects legacy `client`, `client-kmp`, or both.
2. Add shared protocol messages first if signaling changes.
3. Keep browser/native WebRTC interop platform-local.
4. Ensure tracks are added before offer creation.
5. Be explicit about renegotiation and cleanup when tracks stop.

## Testing Guidance

Minimum expected checks by change type:

- Protocol changes: `common` serialization tests.
- Server room behavior: server unit tests if practical, otherwise at least `.\gradlew.bat :server:jvmTest`.
- KMP state/handler logic: `client-kmp` common tests.
- Gradle/config changes: run the affected Gradle task plus `.\gradlew.bat check` when feasible.
- UI/WebRTC changes: run the app and test with at least two peers/tabs/windows when feasible.

If tests cannot be run, say exactly which command was not run and why.

## Codex Editing Rules

- Use `rg` or `rg --files` for search.
- Use `apply_patch` for manual edits.
- Do not edit generated/build output unless the task explicitly requires it.
- Do not edit `libs/webrtc-kmp/` unless the task explicitly targets the included WebRTC fork.
- Keep comments sparse and useful.
- Keep changes scoped. Avoid drive-by refactors during feature work.
- If a file already has user changes, preserve them and work around them carefully.
- If changing protocol, server, and UI together, keep the change coherent across all touched modules in the same turn when feasible.

## Known Constraints

- Current media topology is mesh P2P. Upload bandwidth grows with peer count.
- The signaling server is not a media relay and cannot reduce WebRTC upload fanout.
- Room state disappears on server restart.
- There is no authentication or authorization.
- Some AI analysis docs predate the `client-kmp` migration; verify against current source before relying on them.

## Documentation Hygiene

When changing architecture, protocol, module ownership, or run commands, update `AGENTS.md` first.

Also update these when they are relevant to the user request or already part of the changed area:

- `README.md`
- `.github/copilot-instructions.md`
- `.kiro/steering/*`
- `CLAUDE.md` only if the delegation/important notes change

Keep assistant-facing docs concrete and codebase-specific. Avoid generic advice that does not help Codex choose files, preserve invariants, or run verification.
