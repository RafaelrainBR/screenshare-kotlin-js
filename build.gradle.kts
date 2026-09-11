import org.jlleitschuh.gradle.ktlint.KtlintExtension

plugins {
    alias(libs.plugins.kotlinMultiplatform) apply false
    alias(libs.plugins.kotlinJvm) apply false
    alias(libs.plugins.ksp) apply false
    alias(libs.plugins.kotlinSerialization) apply false
    alias(libs.plugins.kotestMultiplatform) apply false
    alias(libs.plugins.ktor) apply false
    alias(libs.plugins.ktlint)
}

group = "com.rafaelrain"
version = "1.0-SNAPSHOT"

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

val npmCommand = if (System.getProperty("os.name").startsWith("Windows")) "npm.cmd" else "npm"

tasks.register<Exec>("desktopBuild") {
    group = "desktop"
    description = "Builds the Kotlin/JS client and the Windows Tauri application."
    workingDir(layout.projectDirectory.dir("desktop-tauri"))
    commandLine(npmCommand, "run", "tauri", "--", "build")
}

tasks.register<Exec>("desktopDev") {
    group = "desktop"
    description = "Runs the Tauri shell with the Kotlin/JS development server."
    workingDir(layout.projectDirectory.dir("desktop-tauri"))
    commandLine(npmCommand, "run", "tauri", "--", "dev")
}
