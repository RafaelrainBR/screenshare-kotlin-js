import org.jetbrains.kotlin.gradle.targets.js.webpack.KotlinWebpackConfig
import org.gradle.api.tasks.Copy
import org.gradle.api.file.DuplicatesStrategy

plugins {
    alias(libs.plugins.kotlinMultiplatform)
    alias(libs.plugins.kotlinSerialization)
}

kotlin {
    js {
        browser {
            val rootDirPath = project.rootDir.path
            val projectDirPath = project.projectDir.path
            commonWebpackConfig {
                outputFileName = "clientApp.js"
                devServer =
                    (devServer ?: KotlinWebpackConfig.DevServer()).apply {
                        port = 8081
                        static =
                            (static ?: mutableListOf()).apply {
                                // Serve sources to debug inside browser
                                add(rootDirPath)
                                add(projectDirPath)
                            }
                    }
            }
        }
        binaries.executable()
    }

    sourceSets {
        commonMain.dependencies {
            implementation(project(":common"))
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.kotlinx.serialization.json)

            implementation(libs.ktor.client.core)
            implementation(libs.ktor.client.cio)
            implementation(libs.ktor.client.contentNegotiation)
            implementation(libs.ktor.client.websockets)
            implementation(libs.ktor.serialization.kotlinx.json)
        }

        jsTest.dependencies {
            implementation(kotlin("test"))
        }
    }
}

// The canonical browser build retains the inert placeholder. Tauri's hook
// explicitly opts in so both its development server and packaged bundle load
// the native bridge without adding a Tauri import to web builds.
if (providers.gradleProperty("screenshare.desktop.bridge").isPresent) {
    tasks.named<Copy>("jsProcessResources") {
        // The Tauri bridge source is added after jsMain resources and replaces
        // the browser-safe placeholder only in this opted-in task.
        duplicatesStrategy = DuplicatesStrategy.INCLUDE
        from(rootProject.layout.projectDirectory.dir("desktop-tauri/dist")) {
            include("desktopBridge.js")
        }
    }
}
