# Tech Stack Analysis

## Core Technology Analysis

### Programming Language(s)
- **Kotlin** – primary language across all modules (v2.3.10)
  - Kotlin/JS used in the `client` module (compiled to JavaScript for the browser)
  - Kotlin/JVM used in `server` and `server-java` modules
  - Kotlin Multiplatform (KMP) used in `common` module (targets JS + JVM)

### Primary Framework
- **Ktor** (v3.4.0) – used for both the HTTP/WebSocket server (server-side CIO engine) and the HTTP/WebSocket client (client-side)

### Secondary / Tertiary Frameworks
- **Kotlin Multiplatform (KMP)** – code sharing between JS client and JVM server via the `common` module
- **kotlinx.serialization** (v1.10.0) – JSON serialization/deserialization for Packet messages
- **kotlinx.coroutines** (v1.10.2) – asynchronous programming across client and server
- **KSP** (Kotlin Symbol Processing, v2.2.21-2.0.5) – annotation processing (used in `common`)
- **TailwindCSS + DaisyUI** – CSS utility framework and component library for the browser UI (included via CDN in `index.html`)
- **WebRTC (browser APIs)** – peer-to-peer audio and screen-sharing via `RTCPeerConnection`

### State Management Approach
- No centralized state management library; state is managed through plain Kotlin objects:
  - `Session` class on the client holds all local session state (username, roomId, peer connections, etc.)
  - `Room` class on the server holds all in-memory room state (users, message history)
  - `PeerConnections`, `VoiceChat`, `ScreenSharing` are stateful service objects on the client

### Other Relevant Technologies
- **Logback** (v1.5.31) – SLF4J logging on JVM
- **Kotest** (v6.1.3) – multiplatform test framework with JUnit 5 runner on JVM
- **KtLint** (v14.0.1 plugin / v1.8.0 engine) – Kotlin code formatting/linting
- **Kotlin Wrappers** (v0.0.1-pre.806) – Kotlin type bindings for browser APIs (`org.w3c.dom`, `kotlinx.browser`)
- **Webpack** – bundling Kotlin/JS output (via Kotlin/JS Gradle integration, output file: `clientApp.js`)
- **Docker / Fly.io** – deployment infra (`server-java/Dockerfile`, `docker-compose.yml`, `fly.toml`)
- **Java 21** toolchain – JVM target for the server

---

## Domain Specificity Analysis

### Problem Domain
**Real-time collaborative screen-sharing and voice-chat platform** that runs entirely in the browser. Users join named "rooms" to share their screen and communicate via text chat and microphone audio.

### Core Concepts
- **Room-based collaboration** – multiple users join a room identified by a short ID
- **WebRTC peer-to-peer connections** – screen video and microphone audio are transmitted directly between browsers using WebRTC (`RTCPeerConnection`, ICE, SDP offer/answer)
- **WebSocket signaling** – a Ktor WebSocket server acts as the signaling channel for WebRTC negotiation (ICE candidates, SDP descriptions) and for chat/presence events
- **Packet protocol** – a sealed Kotlin class `Packet` defines all client↔server messages (join, chat, ICE, SDP, screen-share start/stop, mute/unmute) with `@Serializable` annotations and `@SerialName` compact keys
- **MediaStream management** – browser `MediaDevices.getDisplayMedia()` (screen capture) and `getUserMedia()` (microphone) are wrapped in Kotlin decorators

### Supported User Interactions
- Joining a room with a username (and optional room code)
- Sending/receiving text chat messages
- Starting and stopping screen sharing (broadcast to all peers in the room)
- Toggling microphone mute/unmute
- Viewing a real-time participant list with speaking indicators

### Primary Data Types and Structures
- `Packet` – sealed class hierarchy defining the signaling protocol
- `ChatMessage` – serializable data class holding username, content, timestamp
- `SocketUser` – serializable data class holding socketId, username, roomId, isMuted
- `Room` / `RoomUser` – server-side in-memory models
- `Session` – client-side aggregation of local state and service references
- `RTCPeerConnectionDecorator` – Kotlin wrapper around the dynamic JS WebRTC API

---

## Application Boundaries

### Features Clearly Within Scope
- Multi-user rooms with presence (join/leave events)
- Text chat with history replay on join
- Screen capture and broadcast via WebRTC
- Microphone audio via WebRTC with mute indicator
- Themed UI (DaisyUI `night` default, switchable)
- Serverless deployment via Fly.io / Docker

### Architecturally Inconsistent Feature Types
- Native mobile apps (project is Kotlin/JS browser-only on the client side; no Android/iOS targets)
- Persistent storage / databases (the server is fully in-memory; no ORM or DB dependencies)
- REST API endpoints (only WebSocket communication is implemented)
- Video recording or playback (no media recording APIs used)
- Authentication / user accounts (no auth layer exists)

### Specialized Libraries / Domain Constraints
- WebRTC browser API is accessed exclusively through `RTCPeerConnectionDecorator` and `DisplayMediaDecorator` wrappers (all `dynamic` interop is isolated in `decorators/`)
- All client↔server communication goes through the `Packet` sealed class – new messages must be added there and handled in both `Room.consumePacket()` (server) and `SessionHandler.handlePacket()` (client)
- The `common` module is the sole shared code boundary between client and server; it must remain Kotlin Multiplatform compatible (no JVM-only or JS-only APIs)
