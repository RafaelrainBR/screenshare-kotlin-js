# Domain: deployment

## Overview
The application is packaged as a self-contained runnable JVM application. The Kotlin/JS client is bundled by Webpack and embedded into the server JAR as static resources before deployment. The final artifact runs inside Docker and is deployed to Fly.io.

---

## Files
- `server-java/build.gradle.kts`
- `server/build.gradle.kts`
- `server-java/Dockerfile`
- `server-java/docker-compose.yml`
- `server-java/fly.toml`

---

## Build Pipeline

### Step 1: Bundle the Client
In `server/build.gradle.kts`:
```kotlin
val clientBuildDir = project(":client").layout.buildDirectory.dir("dist/js/productionExecutable")

tasks.register<Copy>("copyClientToServer") {
    dependsOn(":client:jsBrowserDistribution")
    from(clientBuildDir)
    into("src/jvmMain/resources/static")
}

tasks.named("jvmProcessResources") {
    dependsOn("copyClientToServer")
}
```

The Kotlin/JS client is compiled to `clientApp.js` (configured via `outputFileName` in `client/build.gradle.kts`) and copied to `server/src/jvmMain/resources/static/`.

### Step 2: Server Entry Point
`server-java` is the runnable module. It depends on `:server` and sets the main class:
```kotlin
// server-java/build.gradle.kts
plugins {
    alias(libs.plugins.kotlinJvm)
    alias(libs.plugins.ktor)
}
application {
    mainClass.set("screenshare.server.ApplicationKt")
}
dependencies {
    implementation(project(":server"))
}
```

The `ktor` plugin creates a fat JAR (`shadowJar`) in `server-java/build/libs/`.

### Step 3: Static Resource Serving
In `Application.module()` (server):
```kotlin
routing {
    staticResources("/", "static")
    webSocket("/") { ... }
}
```

`index.html` and `clientApp.js` are served from the root path.

---

## Docker

`server-java/Dockerfile` packages the fat JAR:
- Base image: Java 21
- Exposes port 8080
- Runs the shadow JAR

`docker-compose.yml` is provided for local multi-container development.

---

## Fly.io Configuration (`fly.toml`)
```toml
app = 'screen-share'
primary_region = 'gru'   # São Paulo

[http_service]
  internal_port = 8080
  force_https = true
  auto_stop_machines = 'stop'
  auto_start_machines = true
  min_machines_running = 0
```

- HTTPS is forced at the Fly.io edge; the server itself speaks plain HTTP/WS on port 8080.
- `force_https = true` means the client detects `https:` and uses `wss://` for the WebSocket (handled in `Main.kt`).
- `auto_stop_machines = 'stop'` means the machine stops when idle (no connections) to reduce cost.
- Region: `gru` (Guarulhos, São Paulo, Brazil).

---

## Local Dev

Ktor development mode is enabled via `gradle.properties`:
```properties
io.ktor.development=true
```

Run the dev server:
```bash
./gradlew :server-java:run
```

The webpack dev server for hot-reload can be started separately:
```bash
./gradlew :client:jsBrowserDevelopmentRun
```
