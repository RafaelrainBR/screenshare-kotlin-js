# Claude Code Guide

This repository is optimized for Codex through `AGENTS.md`.

Read `AGENTS.md` first. It is the canonical project guide and should be treated as the source of truth. This file exists only as a compatibility entry point for Claude Code.

## Project Context

Screenshare is a Kotlin Multiplatform app for real-time screen sharing, voice chat, and text chat.

- `common/`: shared packet protocol and DTOs.
- `server/`: Ktor/JVM WebSocket signaling server.
- `server-java/`: runnable/deployable JVM wrapper.
- `client/`: legacy Kotlin/JS browser client using DOM and DaisyUI.
- `client-kmp/`: newer Compose Multiplatform client for Desktop JVM and wasmJs.
- `libs/webrtc-kmp/`: included WebRTC dependency/fork; avoid editing unless explicitly requested.

The server handles signaling, presence, chat history, and SDP/ICE relay. Media is WebRTC peer-to-peer mesh. The server is not an SFU and does not relay screen/audio streams.

## Core Rules

- Prefer `client-kmp` for new app architecture and future rewrite work.
- Use `client` for the current legacy browser app and server-bundled static UI.
- Treat `common/src/commonMain/kotlin/screenshare/common/Packet.kt` as the protocol source of truth.
- For protocol changes, update `Packet.getSide()`, server handling, relevant client handlers, and serialization tests.
- Keep common code platform-neutral.
- Keep legacy browser dynamic WebRTC access inside `client/src/jsMain/kotlin/decorators/`.
- Keep KMP platform WebRTC details behind `WebRtcManager` expect/actual implementations.
- Keep UI text in Brazilian Portuguese where user-facing strings already are Portuguese.

## Useful Commands

On Windows PowerShell:

- `.\gradlew.bat :server-java:run`
- `.\gradlew.bat :client-kmp:run`
- `.\gradlew.bat :client-kmp:wasmJsBrowserDevelopmentRun --continuous`
- `.\gradlew.bat :client:jsBrowserDistribution`
- `.\gradlew.bat :server-java:buildFatJar`
- `.\gradlew.bat check`
- `.\gradlew.bat ktlintCheck`

Run focused Gradle checks for the modules you change, and report any checks you could not run.
