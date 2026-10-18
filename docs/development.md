# Development

[Documentation](README.md) · [Desktop development](../desktop-tauri/README.md)

## Prerequisites

- **JDK 21**, matching the configured JVM toolchain.
- Git. Use the included Gradle wrapper; a separate Gradle installation is unnecessary.
- Internet access for the first dependency download. Gradle provisions Node/Yarn for Kotlin/JS.
- Desktop development additionally needs Node/npm, Rust and platform tools; see the [desktop guide](../desktop-tauri/README.md).

Dependency versions belong to [`libs.versions.toml`](../libs.versions.toml) and the desktop npm/Cargo manifests.

## Browser and server

From the repository root:

```bash
./gradlew :server-java:run
```

On PowerShell, replace `./gradlew` with `.\gradlew.bat`. Open [localhost:8080](http://localhost:8080). This builds the production browser bundle, copies it into the server's static resources, and starts Ktor. Restart/rebuild after frontend changes; this command does not provide frontend hot reload.

`PORT` changes the Ktor server port. The browser uses its page's host/port for signaling. The standalone Webpack server (`:client:jsBrowserDevelopmentRun`, port 8081) has no signaling proxy configured; use the full Ktor server for browser room tests. The desktop development shell explicitly selects the local signaling server separately.

## Build

```bash
# Browser assets: client/build/dist/js/productionExecutable
./gradlew :client:jsBrowserDistribution

# Includes browser assets: server-java/build/libs/server-java-all.jar
./gradlew :server-java:buildFatJar
```

The standalone JAR runs with:

```bash
java -jar server-java/build/libs/server-java-all.jar
```

On hosts where the default Gradle cache is not writable, set `GRADLE_USER_HOME` before invoking the wrapper. The desktop frontend hook uses the repository-local ignored `.gradle` directory automatically.

## Tests and checks

Run the tests used by the release workflow:

```bash
./gradlew :client:jsTest :common:allTests :server:jvmTest
node vendor/braces/test.cjs
```

`node` is required on PATH for the standalone vendor check. Client tests cover capture lifecycle, media gates, share profiles, negotiation and WebSocket behavior. Common tests cover serialized packets; server tests cover room/media behavior.

For Gradle's complete verification tasks, including lint:

```bash
./gradlew check
```

Native desktop checks run from `desktop-tauri/src-tauri`:

```bash
cargo fmt --check
cargo test --release
```

These checks do not establish that a physical camera, microphone, OS picker, installer or remote audio path works. Validate changes to capture on real hardware with two peers. Existing local browser QA scripts are in [`desktop-tauri/scripts/`](../desktop-tauri/scripts); their prerequisites and assertions are documented in their source. For documentation captures, use the [screenshot workflow](images/README.md).

## Code map

The [README module map](../README.md#how-it-works) shows the application boundaries. Start with `client/src/jsMain/kotlin/services/` for session/media logic, `client/src/jsMain/kotlin/ui/` for UI handlers/mutations, and `client/src/jsMain/resources/` for the interface, styles, fonts and brand assets.

Browser builds retain an inert `desktopBridge.js`. Only the Tauri build hook opts into the native bridge with `-Pscreenshare.desktop.bridge`. Keep that boundary when changing capture behavior.
