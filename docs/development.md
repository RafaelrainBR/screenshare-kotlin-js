# Development

[Documentation](README.md) · [Desktop development](../desktop-native/README.md)

## Prerequisites

- **JDK 21**, matching the configured JVM toolchain.
- Git. Use the included Gradle wrapper; a separate Gradle installation is unnecessary.
- Internet access for the first dependency download. Gradle provisions Node/Yarn for Kotlin/JS.
- Desktop development additionally needs Rust and platform tools; see the [desktop guide](../desktop-native/README.md).

Dependency versions belong to [`libs.versions.toml`](../libs.versions.toml) and the desktop Cargo manifests.

## Browser and server

From the repository root:

```bash
./gradlew :server-java:run
```

On PowerShell, replace `./gradlew` with `.\gradlew.bat`. Open [localhost:8080](http://localhost:8080). This builds the production browser bundle, copies it into the server's static resources, and starts Ktor. Restart/rebuild after frontend changes; this command does not provide frontend hot reload.

`PORT` changes the Ktor server port. The browser uses its page's host/port for signaling. The standalone Webpack server (`:client:jsBrowserDevelopmentRun`, port 8081) has no signaling proxy configured; use the full Ktor server for browser room tests. The desktop app connects to the public server unless you start it with `SCREENSHARE_WS=ws://localhost:8080/`.

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

On hosts where the default Gradle cache is not writable, set `GRADLE_USER_HOME` before invoking the wrapper.

## Tests and checks

Run the checks that [`dependency-security.yml`](../.github/workflows/dependency-security.yml) runs on every pull request outside `desktop-native/`:

```bash
./gradlew :client:jsTest :common:allTests :server:jvmTest :server-java:buildFatJar ktlintCheck
node vendor/braces/test.cjs
node --test scripts/build-presentation.test.mjs
node scripts/build-presentation.mjs --check
```

`node` is required on PATH for the script checks. After editing `site/`, regenerate the public pages with `node scripts/build-presentation.mjs` before the check; see [web publication](presence/web-publication.md). Client tests cover capture lifecycle, media gates, share profiles, negotiation and WebSocket behavior. Common tests cover serialized packets; server tests cover room/media behavior.

For Gradle's complete verification tasks, including lint:

```bash
./gradlew check
```

Desktop checks, as [`native-check.yml`](../.github/workflows/native-check.yml) runs them on macOS for changes to the desktop folders:

```bash
cargo test --release --locked --manifest-path desktop-capture/Cargo.toml
cd desktop-native
cargo test --release --locked
```

These checks do not establish that a physical camera, microphone, OS picker, installer or remote audio path works. Validate changes to capture on real hardware with two peers. For documentation captures, use the [screenshot workflow](images/README.md).

## Code map

The [README module map](../README.md#how-it-works) shows the application boundaries. Start with `client/src/jsMain/kotlin/services/` for session/media logic, `client/src/jsMain/kotlin/ui/` for UI handlers/mutations, and `client/src/jsMain/resources/` for the interface, styles, fonts and brand assets.
