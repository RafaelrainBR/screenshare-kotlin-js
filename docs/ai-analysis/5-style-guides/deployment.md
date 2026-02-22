# Style Guide: deployment

## Unique Conventions

### Fat JAR via Ktor Gradle Plugin
The `server-java` module uses `alias(libs.plugins.ktor)` which provides the `buildFatJar` / `shadowJar` task. The entry point is `screenshare.server.ApplicationKt`.

### Client Compiled and Embedded Before JVM Build
`server/build.gradle.kts` defines a `copyClientToServer` task that copies `client/build/dist/js/productionExecutable/` into `server/src/jvmMain/resources/static/`. This task is a dependency of `jvmProcessResources`.

### Same Port for WebSocket and Static Files
Port 8080 serves both the Ktor WebSocket endpoint and the static frontend. No separate reverse proxy inside the container.

### Fly.io Region: gru (São Paulo)
`primary_region = 'gru'` in `fly.toml`. New deployments should keep this unless intentionally changing the target region.

### Auto-stop on Idle
`auto_stop_machines = 'stop'` and `min_machines_running = 0` reduce cost when no users are connected.

### Docker Base Image: Java 21
`server/build.gradle.kts` sets `languageVersion.set(JavaLanguageVersion.of(21))`. The `Dockerfile` must use a Java 21 base image.

### Development Mode Toggle
`io.ktor.development=true` in `gradle.properties` enables Ktor development mode (hot-reload). This is a project-level default applicable during local development.
