import org.jetbrains.compose.desktop.application.dsl.TargetFormat
import org.jetbrains.kotlin.gradle.ExperimentalWasmDsl

plugins {
    alias(libs.plugins.kotlinMultiplatform)
    alias(libs.plugins.kotlinSerialization)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.compose.compiler)
    alias(libs.plugins.ksp)
    alias(libs.plugins.kotestMultiplatform)
}

kotlin {
    jvm("desktop")

    @OptIn(ExperimentalWasmDsl::class)
    wasmJs {
        browser()
        binaries.executable()
    }

    sourceSets {
        val desktopMain by getting

        commonMain.dependencies {
            implementation(project(":common"))

            implementation(compose.runtime)
            implementation(compose.foundation)
            implementation(compose.material3)
            implementation(compose.materialIconsExtended)
            implementation(compose.ui)
            implementation(compose.components.resources)

            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.kotlinx.serialization.json)

            implementation(libs.ktor.client.core)
            implementation(libs.ktor.client.websockets)
            implementation(libs.ktor.serialization.kotlinx.json)
            implementation(libs.kotlinx.datetime)
        }

        desktopMain.dependencies {
            implementation(compose.desktop.currentOs)
            implementation(libs.kotlinx.coroutines.swing)
            implementation(libs.ktor.client.okhttp)
            implementation(libs.webrtc.kmp)
            implementation("org.slf4j:slf4j-simple:2.0.9")

            // Platform-native WebRTC DLLs/SOs required at runtime
            val osName = System.getProperty("os.name")
            val hostOS = when {
                osName == "Mac OS X" -> "macos"
                osName.startsWith("Win") -> "windows"
                osName.startsWith("Linux") -> "linux"
                else -> error("Unsupported OS: $osName")
            }
            val hostArch = when (val arch = System.getProperty("os.arch").lowercase()) {
                "amd64" -> "x86_64"
                "x86_64" -> "x86_64"
                "aarch64" -> "aarch64"
                else -> arch
            }
            runtimeOnly("dev.onvoid.webrtc:webrtc-java:0.14.0:$hostOS-$hostArch")
        }

        wasmJsMain.dependencies {
            // Ktor auto-configura engine para wasmJs
        }

        commonTest.dependencies {
            implementation(libs.kotest.assertions.core)
            implementation(libs.kotest.framework.engine)
            implementation(libs.kotlinx.coroutines.core)
        }

        val desktopTest by getting {
            dependencies {
                implementation(libs.kotest.runner.junit5)
            }
        }
    }
}

tasks.withType<Test>().configureEach {
    useJUnitPlatform()
}

compose.desktop {
    application {
        mainClass = "MainKt"

        nativeDistributions {
            targetFormats(TargetFormat.Msi, TargetFormat.Deb)
            packageName = "screenshare-desktop"
            packageVersion = "1.0.2"
            description = "Real-time screen sharing and voice chat"
            vendor = "screenshare"

            windows {
                shortcut = true         // atalho na área de trabalho
                menuGroup = "ScreenShare" // entrada no menu iniciar
                perUserInstall = true   // instala sem precisar de admin
                dirChooser = true       // permite escolher a pasta de instalação
                upgradeUuid = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
            }
        }
    }
}
