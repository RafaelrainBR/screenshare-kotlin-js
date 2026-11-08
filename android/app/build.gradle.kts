import java.io.File

plugins {
    alias(libs.plugins.androidApplication)
    alias(libs.plugins.composeCompiler)
}

// android-vX.Y.Z[-rc.N] → X·1000000 + Y·10000 + Z·100 + N, the final release 99: each rc installs
// over the previous one and the final over all of them (docs/design/android-plan.md, Atualização).
val appVersion = (findProperty("appVersion") as String?) ?: "0.0.0-rc.1"

fun versionCodeOf(version: String): Int {
    val match = Regex("""^(\d+)\.(\d+)\.(\d+)(?:-rc\.(\d+))?$""").matchEntire(version)
        ?: error("appVersion must look like 1.2.3 or 1.2.3-rc.4: $version")
    val (major, minor, patch, rc) = match.destructured
    require(minor.toInt() < 100 && patch.toInt() < 100 && (rc.isEmpty() || rc.toInt() in 1..98)) { "version part out of range: $version" }
    return major.toInt() * 1_000_000 + minor.toInt() * 10_000 + patch.toInt() * 100 + (if (rc.isEmpty()) 99 else rc.toInt())
}
check(versionCodeOf("1.2.3-rc.4") == 1_020_304 && versionCodeOf("1.2.3") == 1_020_399 && versionCodeOf("0.1.0-rc.1") < versionCodeOf("0.1.0"))

// The Rust engine (android/engine) through cargo-ndk. Debug also builds x86_64 for the emulator.
val rustAbis = (findProperty("abis") as String?)?.split(",") ?: listOf("arm64-v8a", "armeabi-v7a", "x86_64")

android {
    namespace = "com.rafaelrain.screenshare"
    compileSdk = 37
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "com.rafaelrain.screenshare"
        minSdk = 29
        targetSdk = 36
        versionCode = versionCodeOf(appVersion)
        versionName = appVersion
        // Only the ABIs the engine is built for: libraries (androidx, Sentry) also ship x86/x86_64, and a
        // device that picks one of those would not find libscreenshare_android.so.
        ndk { abiFilters += rustAbis }
        // Only CI release builds have it (secret SENTRY_DSN_ANDROID); local and PR builds send nothing.
        buildConfigField("String", "SENTRY_DSN", "\"${System.getenv("SENTRY_DSN_ANDROID").orEmpty()}\"")
    }

    signingConfigs {
        create("release") {
            val keystore = System.getenv("ANDROID_KEYSTORE_FILE")
            if (keystore != null) {
                storeFile = file(keystore)
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEY_ALIAS")
                keyPassword = System.getenv("ANDROID_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        debug {
            // The emulator test reaches a local server through adb reverse (the engine allows ws:// only on localhost).
            buildConfigField("String", "SERVER", "\"${(findProperty("server") as String?) ?: "wss://screen-share.fly.dev/"}\"")
        }
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.getByName("release").takeIf { it.storeFile != null } ?: signingConfigs.getByName("debug")
            buildConfigField("String", "SERVER", "\"wss://screen-share.fly.dev/\"")
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    sourceSets["main"].jniLibs.directories.add(layout.buildDirectory.dir("rust/jniLibs").get().asFile.path)

    packaging {
        // 16 KB pages: libraries stay uncompressed and aligned in the APK.
        jniLibs.useLegacyPackaging = false
    }
}

val rustTarget = System.getenv("CARGO_TARGET_DIR") ?: File(rootDir, "engine/target").path
val cargoBuild by tasks.registering(Exec::class) {
    val release = gradle.startParameter.taskNames.any { it.contains("Release", ignoreCase = true) }
    workingDir = file("../engine")
    environment("CARGO_TARGET_DIR", rustTarget)
    environment("RUSTFLAGS", "-C link-arg=-Wl,-z,max-page-size=16384")
    commandLine(
        listOf("cargo", "ndk", "-P", "29", "-o", layout.buildDirectory.dir("rust/jniLibs").get().asFile.path) +
            rustAbis.flatMap { listOf("-t", it) } +
            listOf("build") + (if (release) listOf("--release") else emptyList()),
    )
    // libwebrtc's Java side (livekit.org.webrtc.*), which webrtc-sys copies next to its build output.
    doLast {
        val jar = File(rustTarget).walk().filter { it.name == "libwebrtc.jar" }.maxByOrNull { it.lastModified() }
            ?: error("libwebrtc.jar not found under $rustTarget")
        jar.copyTo(layout.buildDirectory.file("rust/libwebrtc.jar").get().asFile, overwrite = true)
    }
}
tasks.named("preBuild") { dependsOn(cargoBuild) }

dependencies {
    implementation(files(layout.buildDirectory.file("rust/libwebrtc.jar")))
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.material3)
    implementation(libs.compose.ui)
    implementation(libs.activity.compose)
    implementation(libs.core.ktx)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.lifecycle.service)
    implementation(libs.camera.camera2)
    implementation(libs.camera.lifecycle)
    implementation(libs.sentry.android)
    testImplementation(libs.junit)
    // The real org.json: android.jar only has stubs that throw in unit tests.
    testImplementation(libs.json)
}
