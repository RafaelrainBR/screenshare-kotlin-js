rootProject.name = "screenshare"

includeBuild("libs/webrtc-kmp") {
    dependencySubstitution {
        substitute(module("com.shepeliev:webrtc-kmp")).using(project(":webrtc-kmp"))
    }
}

pluginManagement {
    repositories {
        google {
            mavenContent {
                includeGroupAndSubgroups("androidx")
                includeGroupAndSubgroups("com.android")
                includeGroupAndSubgroups("com.google")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        google {
            mavenContent {
                includeGroupAndSubgroups("androidx")
                includeGroupAndSubgroups("com.android")
                includeGroupAndSubgroups("com.google")
            }
        }
        mavenCentral()
    }

    versionCatalogs {
        create("kotlinWrappers") {
            val wrappersVersion = "0.0.1-pre.806"
            from("org.jetbrains.kotlin-wrappers:kotlin-wrappers-catalog:$wrappersVersion")
        }
        create("libs") {
            from(files("libs.versions.toml"))
        }
    }
}

include(":client")
include(":client-kmp")
include(":common")
include(":server")
include(":server-java")
