# GitHub Copilot Instructions

This repository is optimized for Codex through `AGENTS.md`.

Use `AGENTS.md` at the repository root as the canonical guide for architecture, module ownership, coding patterns, and verification commands. The notes below are only condensed compatibility context for inline suggestions.

## Project Context

- This is a Kotlin Multiplatform screen-sharing app with a Ktor/JVM WebSocket signaling server.
- `common/` defines the shared packet protocol.
- `server/` handles in-memory rooms, text chat, presence, and WebRTC SDP/ICE relay.
- `server-java/` packages and runs the JVM server.
- `client/` is the legacy Kotlin/JS browser client using DOM/DaisyUI.
- `client-kmp/` is the newer Compose Multiplatform client for Desktop JVM and wasmJs.
- `libs/webrtc-kmp/` is an included WebRTC dependency/fork; avoid editing it unless explicitly requested.

The server does not relay media. Screen and audio streams use WebRTC peer-to-peer mesh connections.

## Coding Rules

For protocol changes, always update `Packet.kt`, `Packet.getSide()`, server handling, relevant client packet handlers, and serialization tests.

For new app/client work, prefer `client-kmp` unless the prompt specifically targets the legacy browser app.

Keep `common/commonMain` platform-neutral.

Keep legacy browser dynamic WebRTC access inside `client/src/jsMain/kotlin/decorators/`.

Keep KMP platform WebRTC details behind `WebRtcManager` expect/actual implementations.

Use version catalog aliases and keep dependency versions in `libs.versions.toml`.

User-facing UI strings should remain Brazilian Portuguese where the existing UI uses Portuguese.
