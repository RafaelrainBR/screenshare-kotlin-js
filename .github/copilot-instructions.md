# GitHub Copilot Instructions — screenshare-kotlin-js

## Overview

This file enables AI coding assistants to generate features aligned with this project's architecture and style. All patterns described here are derived exclusively from actual, observed conventions in the codebase — not invented best practices.

This project is a **real-time browser-based screen-sharing and voice-chat platform** built with Kotlin Multiplatform. A Ktor/JVM server provides WebSocket signaling; a Kotlin/JS client handles WebRTC, UI, and media capture.

---

## Tech Stack Summary

| Layer | Technology |
|---|---|
| Language | Kotlin 2.3.10 (JS + JVM, via KMP) |
| Server framework | Ktor 3.4.0 (CIO engine) |
| Client runtime | Kotlin/JS (Webpack, targets browser) |
| Serialization | kotlinx.serialization 1.10.0 |
| Async | kotlinx.coroutines 1.10.2 |
| UI styling | TailwindCSS + DaisyUI (CDN, `night` theme default) |
| Media | WebRTC browser APIs (`RTCPeerConnection`, `getDisplayMedia`, `getUserMedia`) |
| Tests | Kotest 6.1.3 (FunSpec, multiplatform) |
| Linting | KtLint 1.8.0 |
| Deployment | Ktor fat JAR → Docker → Fly.io (`gru` region) |

---

## File Category Reference

### `build-config`
_Gradle build files, version catalog, wrapper_

Examples: `build.gradle.kts`, `settings.gradle.kts`, `libs.versions.toml`, `client/build.gradle.kts`

Conventions:
- All versions in `libs.versions.toml` — never inline version strings in `.kts` files
- Plugins applied with `alias(libs.plugins.X)`; root-level plugins use `apply false`; subprojects apply via `subprojects { apply(plugin = "...") }`
- KMP source sets declare dependencies inside `kotlin { sourceSets { ... } }`
- The `copyClientToServer` task in `server/build.gradle.kts` embeds the Webpack bundle into the server JAR

### `entrypoints`
_Application entry points_

Examples: `client/src/jsMain/kotlin/Main.kt`, `server/src/jvmMain/kotlin/screenshare/server/Application.kt`

Conventions:
- Client `main()`: creates `WebsocketService` from `window.location`, creates `CoroutineScope(Dispatchers.Main + SupervisorJob())`, calls `registerUIHandlers()`
- Server: `fun main(args)` delegates to `EngineMain.main(args)`; module logic is in `fun Application.module()`

### `shared-protocol`
_Sealed `Packet` class — the wire protocol_

Examples: `common/src/commonMain/kotlin/screenshare/common/Packet.kt`

Conventions:
- Every message is a `data class` inside `sealed class Packet`
- All classes annotated `@Serializable @SerialName("kebab-case-type")`
- Short field `@SerialName`: `rid`=roomId, `sid`=socketId/senderId, `tid`=targetId, `msg`=message, `ice`=candidate
- Every packet classified as `CLIENT` or `SERVER` in `Packet.getSide()`

### `shared-models`
_Data transfer objects shared between client and server_

Examples: `common/src/commonMain/kotlin/screenshare/common/ChatMessage.kt`

Conventions:
- Flat `@Serializable data class`; no business logic
- Timestamps as `Long` (epoch ms)
- Defaults for optional state: `isMuted = true`

### `server-routing`
_Ktor routing and WebSocket endpoint setup_

Examples: `server/src/jvmMain/kotlin/screenshare/server/Application.kt`

Conventions:
- One WebSocket endpoint at `"/"`; `staticResources("/", "static")` for frontend
- `JoinRoom` handled directly in endpoint; all other packets routed to `Room.consumePacket()`
- Client IP resolved via `X-Forwarded-For` header with fallback

### `server-room-management`
_In-memory room and user lifecycle_

Examples: `server/src/jvmMain/kotlin/screenshare/server/Room.kt`

Conventions:
- Rooms created lazily with `computeIfAbsent`, removed when empty
- Chat history replayed to newly joined users
- After structural changes (join/leave): broadcast `UserConnected`/`UserDisconnected` then broadcast fresh `UserList`
- Logging via `LoggerFactory.getLogger(Room::class.java)`, format: `"Room [$id] ..."`

### `client-services`
_Stateful client service classes_

Examples: `services/Session.kt`, `services/VoiceChat.kt`, `services/ScreenSharing.kt`

Conventions:
- Plain classes, no DI framework; dependencies are constructor-injected
- `Session` is `CoroutineScope by coroutineScope`
- Action methods: `fun handleX() = launch { ... }` (return `Job`)
- Cross-service effects passed as lambda callbacks (`recreatePeerConnections: () -> Unit`), not direct references

### `client-websocket`
_WebSocket connection management_

Examples: `services/WebsocketService.kt`

Conventions:
- Single private `sendPacket(Packet)` method; all public methods delegate to it
- Handler injected as `(Session, Packet, CoroutineScope) -> Unit`
- `onClose()` callback for UI notification on disconnect

### `client-webrtc`
_WebRTC peer connection management_

Examples: `services/PeerConnections.kt`

Conventions:
- One `RTCPeerConnectionDecorator` per remote peer (`socketId` → decorator)
- `addTracksIfNotPresent()` called before every offer/renegotiation
- `isInitiator: Boolean` determines offer vs. answer role
- Stream type inferred from track type: video tracks → screen, audio-only → mic

### `decorators`
_Kotlin wrappers around browser dynamic APIs_

Examples: `decorators/RTCPeerConnectionDecorator.kt`, `decorators/DisplayMediaDecorator.kt`

Conventions:
- All `dynamic` access to WebRTC/MediaDevices APIs stays inside `decorators/`
- Decorator class holds `dynamic` object privately; exposes typed methods
- Factory via `companion object { fun create() }`
- Missing methods on typed Kotlin objects become extension functions (e.g., `MediaDevices.getDisplayMedia()`)

### `ui-dom-access`
_Centralized DOM element references_

Examples: `ui/Elements.kt`

Conventions:
- All element bindings in `object Elements` using typed `getElement<T>(id)` helper
- IDs are `kebab-case`; properties are `camelCase` semantic names
- No `document.getElementById()` calls outside `Elements.kt`

### `ui-mutations`
_DOM modification operations_

Examples: `ui/InterfaceMutations.kt`, `ui/mutations/UserListMutations.kt`

Conventions:
- `object` singletons for mutation grouping
- Visibility via `classList.add/remove("hidden")`
- DaisyUI component classes: `chat`, `chat-end`, `chat-start`, `chat-bubble`, `chat-bubble-primary`, `avatar`, `ring-2`, `ring-success`
- UI text in Brazilian Portuguese; own messages shown as `"Você"`, system messages as `"Sistema"`

### `ui-event-handlers`
_Browser event listener registration_

Examples: `ui/InterfaceHandlers.kt`

Conventions:
- All listeners registered via `registerUIHandlers(...)` called once from `main()`
- Each action: private `setupXyzHandler(callback)` function
- `e.preventDefault()` on every click handler
- Input values always `.trim()`-ed; `messageInput` cleared after send

### `packet-handler`
_Client-side incoming packet dispatch_

Examples: `services/SessionHandler.kt`

Conventions:
- Top-level `fun handlePacket(session, packet, coroutineScope)` — not a class method
- Whole dispatch in `runCatching { }.onFailure { println(...) }`
- Each packet type in its own private top-level `handleX()` function
- Empty `->  {}` branches for intentionally-ignored packets

### `utility-functions`
_Pure utility functions_

Examples: `client/src/jsMain/kotlin/Util.kt`

Conventions:
- Top-level functions; no wrapper class
- String extensions: `getUsernameInitials()`, `sanitizeHTML()`
- Room IDs: `generateRandomRoomId().take(8)` using `kotlin.uuid.Uuid.random()`

### `html-templates`
_Single-page HTML template_

Examples: `client/src/jsMain/resources/index.html`

Conventions:
- One file, two screens toggled by `hidden` class
- TailwindCSS + DaisyUI via CDN; no local CSS build
- Default `data-theme="night"` on `<html>`; `lang="pt-BR"`
- New interactive elements need matching `id` attributes and entries in `ui/Elements.kt`

### `tests`
_Kotest multiplatform tests_

Examples: `common/src/commonTest/kotlin/screenshare/common/MessageSpec.kt`

Conventions:
- Class names end in `Spec`; extend `FunSpec`
- `withData(nameFn = { "should ... [${it.second}]" }, ...)` for parameterized cases
- Test data as `json to expectedObject` pairs
- JSON helper strings in `private companion object` functions
- `shouldBe` for assertions

### `deployment`
_Docker and Fly.io configuration_

Examples: `server-java/Dockerfile`, `server-java/fly.toml`

Conventions:
- Fat JAR via Ktor plugin; main class `screenshare.server.ApplicationKt`
- Client Webpack bundle embedded in server JAR via `copyClientToServer` Gradle task
- Port 8080 internal; Fly.io enforces HTTPS at edge
- Region `gru` (São Paulo)

---

## Feature Scaffold Guide

### Adding a New Server→Client Notification

1. Add `data class MyEvent(...)` inside `Packet` in `common/src/commonMain/kotlin/screenshare/common/Packet.kt`
   - `@Serializable @SerialName("my-event")`
   - Add to `SERVER` arm of `getSide()`
2. Trigger it on the server in `Room.consumePacket()` or `Room.notifyX()` via `broadcast(MyEvent(...))` or `user.sendPacket(MyEvent(...))`
3. Handle it on the client in `SessionHandler.kt` by adding a branch to `handlePacket()` and a private `handleMyEvent()` function
4. Update the UI via `InterfaceMutations` or `UserListMutations` methods

### Adding a New Client→Server Action

1. Add `data class MyAction(...)` inside `Packet` — `CLIENT` side
2. Add `suspend fun myAction(...) { sendPacket(Packet.MyAction(...)) }` to `WebsocketService`
3. Add `fun handleMyAction() = launch { websocketService.myAction(...) }` to `Session`
4. Wire to a UI event in `InterfaceHandlers.kt` and add to `registerUIHandlers(...)` if it needs a new button
5. Update `index.html` to add the button element; add it to `Elements.kt`
6. Handle in `Room.consumePacket()` on the server

### Adding a New UI Component

1. Add the HTML element with a unique `kebab-case` `id` to `index.html`
2. Add a reference in `object Elements` in `ui/Elements.kt`
3. Add mutation functions in `InterfaceMutations.kt` or `UserListMutations.kt`
4. Register event listeners in `InterfaceHandlers.kt` and include in `registerUIHandlers(...)`

### Adding a New WebRTC Track Type

1. Capture the stream in a new service class (follow `ScreenSharing` / `VoiceChat` pattern)
2. Wrap any missing browser API calls in `decorators/`
3. Inject the new service into `PeerConnections` via constructor
4. Update `addTracksIfNotPresent()` to include the new stream's tracks
5. Update `onTrack` stream type inference in `createPeerConnection()`

---

## Integration Rules

| Rule | Source |
|---|---|
| All client↔server messages must be `Packet` subclasses in `common` | `signaling-layer` domain |
| `Packet.getSide()` must be updated for every new packet | `shared-protocol` |
| No direct `dynamic` WebRTC access outside `decorators/` | `decorators` style guide |
| New packets handled in `Room.consumePacket()` (server) AND `handlePacket()` (client) | `signaling-layer`, `packet-handler` |
| DOM modifications only in `InterfaceMutations` / `UserListMutations` | `ui-mutations` |
| `document.getElementById` calls only inside `Elements.kt` | `ui-dom-access` |
| Code in `common/commonMain` must be KMP-compatible (no JVM or JS platform APIs) | `multiplatform-common` |
| Chat message history replayed on room join | `room-management` |
| WebRTC offer/answer: always call `addTracksIfNotPresent` before creating an offer | `webrtc-peer-connections` |
| UI text in Brazilian Portuguese | `ui-mutations`, `html-templates` |
| System messages use `username = "Sistema"` | `packet-handler`, `ui-mutations` |

---

## Example Prompt Usage

**Prompt:**
> "Add a feature where users can raise their hand to get the presenter's attention."

**Expected files to create/modify:**

- `common/src/commonMain/kotlin/screenshare/common/Packet.kt`
  — Add `RaiseHand` (client→server) and `HandRaised` / `HandLowered` (server→client) packet subclasses

- `server/src/jvmMain/kotlin/screenshare/server/Room.kt`
  — Handle `RaiseHand` in `consumePacket()`: broadcast `HandRaised(roomId, senderId)`

- `client/src/jsMain/kotlin/ui/Elements.kt`
  — Add `val raiseHandButton = getElement<HTMLButtonElement>("raiseHandBtn")`

- `client/src/jsMain/resources/index.html`
  — Add `<button id="raiseHandBtn">` in the navbar

- `client/src/jsMain/kotlin/ui/InterfaceHandlers.kt`
  — Add `setupRaiseHandHandler(onRaiseHand)` and include in `registerUIHandlers(...)`

- `client/src/jsMain/kotlin/ui/InterfaceMutations.kt`
  — Add `fun showHandRaisedIndicator(socketId: String)` that adds a visual indicator

- `client/src/jsMain/kotlin/ui/mutations/UserListMutations.kt`
  — Add hand-raised icon toggle on the user list item

- `client/src/jsMain/kotlin/services/WebsocketService.kt`
  — Add `suspend fun sendRaiseHand(roomId: String) { sendPacket(Packet.RaiseHand(roomId)) }`

- `client/src/jsMain/kotlin/services/Session.kt`
  — Add `fun handleRaiseHand() = launch { websocketService.sendRaiseHand(localRoomId) }`

- `client/src/jsMain/kotlin/services/SessionHandler.kt`
  — Handle `Packet.HandRaised` and `Packet.HandLowered` in `handlePacket()`

- `common/src/commonTest/kotlin/screenshare/common/MessageSpec.kt`
  — Add serialization round-trip tests for the new packets
