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
