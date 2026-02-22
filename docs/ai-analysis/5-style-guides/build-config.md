# Style Guide: build-config

## Unique Conventions

### Version Catalog (libs.versions.toml)
All dependency versions and plugin versions are declared in `libs.versions.toml` at the project root. Direct version strings in `build.gradle.kts` files are not used.

```toml
[versions]
kotlin = "2.3.10"
ktor = "3.4.0"

[libraries]
ktor-server-core = { group = "io.ktor", name = "ktor-server-core", version.ref = "ktor" }

[plugins]
kotlinMultiplatform = { id = "org.jetbrains.kotlin.multiplatform", version.ref = "kotlin" }
```

### Plugin Application Pattern
All plugins are applied with `alias(libs.plugins.X)` referencing the version catalog. `apply false` on root-level plugins that are applied per-subproject:

```kotlin
// Root build.gradle.kts
plugins {
    alias(libs.plugins.kotlinMultiplatform) apply false
    alias(libs.plugins.ktlint)   // ktlint is applied to all subprojects
}

subprojects {
    apply(plugin = "org.jlleitschuh.gradle.ktlint")
    configure<KtlintExtension> {
        version.set("1.8.0")
        filter {
            exclude { element ->
                val path = element.file.path
                path.contains("\\generated\\") || path.contains("/generated/")
            }
        }
    }
}
```

### Kotlin Wrappers Catalog
A second version catalog named `kotlinWrappers` is created in `settings.gradle.kts` for Kotlin browser bindings:
```kotlin
create("kotlinWrappers") {
    val wrappersVersion = "0.0.1-pre.806"
    from("org.jetbrains.kotlin-wrappers:kotlin-wrappers-catalog:$wrappersVersion")
}
```

### Multiplatform Source Sets
Dependencies are declared inside `kotlin { sourceSets { ... } }` blocks. JVM-only dependencies go in `jvmMain` / `jvmTest`, shared ones in `commonMain` / `commonTest`:
```kotlin
sourceSets {
    commonMain.dependencies { implementation(libs.kotlinx.serialization.json) }
    jvmMain.dependencies { implementation(libs.logback.classic) }
    jvmTest.dependencies { implementation(libs.kotest.runner.junit5) }
}
```

### Task Chaining for Client-Server Build
The `server` module defines a `copyClientToServer` task that depends on `:client:jsBrowserDistribution` and copies the Webpack bundle into `server/src/jvmMain/resources/static/`. `jvmProcessResources` depends on `copyClientToServer`.
