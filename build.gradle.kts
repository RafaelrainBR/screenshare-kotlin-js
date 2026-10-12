import groovy.json.JsonOutput
import groovy.json.JsonSlurper
import org.jetbrains.kotlin.gradle.targets.js.nodejs.NodeJsRootExtension
import org.jetbrains.kotlin.gradle.targets.js.nodejs.NodeJsRootPlugin
import org.jetbrains.kotlin.gradle.targets.js.yarn.YarnPlugin
import org.jetbrains.kotlin.gradle.targets.js.yarn.YarnRootExtension
import org.jlleitschuh.gradle.ktlint.KtlintExtension

buildscript {
    dependencies {
        classpath(platform(libs.jackson.bom))
    }
}

plugins {
    alias(libs.plugins.kotlinMultiplatform) apply false
    alias(libs.plugins.kotlinJvm) apply false
    alias(libs.plugins.ksp) apply false
    alias(libs.plugins.kotlinSerialization) apply false
    alias(libs.plugins.kotestMultiplatform) apply false
    alias(libs.plugins.ktor) apply false
    alias(libs.plugins.shadow) apply false
    alias(libs.plugins.ktlint)
}

group = "com.rafaelrain"
version = "1.0-SNAPSHOT"

plugins.withType<NodeJsRootPlugin> {
    configure<NodeJsRootExtension> {
        versions.mocha.version = libs.versions.mocha.get()
        rootPackageJsonTaskProvider.configure {
            inputs.dir(layout.projectDirectory.dir("vendor/braces"))
            doLast {
                // Kotlin's Yarn resolution DSL only accepts semver, so add the local patch to its generated manifest.
                val jsonFile = rootPackageJsonFile.get().asFile
                val json = JsonSlurper().parse(jsonFile) as MutableMap<String, Any>
                val resolutions = json["resolutions"] as MutableMap<String, String>
                resolutions["braces"] = "file:../../vendor/braces"
                jsonFile.writeText(JsonOutput.prettyPrint(JsonOutput.toJson(json)))
            }
        }
    }
}

plugins.withType<YarnPlugin> {
    configure<YarnRootExtension> {
        resolution("ws", libs.versions.ws.get())
        resolution("qs", libs.versions.qs.get())
    }
}

allprojects {
    configurations.matching { it.name.startsWith("ktlint") }.configureEach {
        resolutionStrategy.eachDependency {
            if (requested.group == "ch.qos.logback") {
                useVersion(libs.versions.logback.get())
            }
        }
    }
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
