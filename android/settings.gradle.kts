// The Android app builds apart from the web and server (../settings.gradle.kts): no AGP on their path.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}
rootProject.name = "screenshare-android"
include(":app")
