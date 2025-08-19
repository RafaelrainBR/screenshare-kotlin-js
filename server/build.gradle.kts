plugins {
    alias(libs.plugins.kotlinMultiplatform)
    alias(libs.plugins.kotlinSerialization)
    alias(libs.plugins.kotestMultiplatform)
}

kotlin {
    targets {
        jvm()
    }

    sourceSets {
        commonMain.dependencies {
            implementation(project(":common"))

            implementation(libs.kotlinx.serialization.json)
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.ktor.server.core)
            implementation(libs.ktor.server.cio)
            implementation(libs.ktor.server.contentNegotiation)
            implementation(libs.ktor.serialization.kotlinx.json)
            implementation(libs.ktor.server.websockets)
        }

        jvmMain {
            dependencies {
                implementation(libs.logback.classic)
            }
        }

        commonTest.dependencies {
            implementation(libs.kotest.assertions.core)
            implementation(libs.kotest.framework.engine)
            implementation(libs.kotest.framework.datatest)
        }

        jvmTest.dependencies {
            implementation(libs.kotest.runner.junit5)
        }
    }
}

tasks.withType<Test>().configureEach {
    useJUnitPlatform()
}

java {
    toolchain {
        languageVersion.set(JavaLanguageVersion.of(21))
    }
}

val clientBuildDir = project(":client").layout.buildDirectory.dir("dist/js/productionExecutable")

tasks.register<Copy>("copyClientToServer") {
    dependsOn(":client:jsBrowserDistribution")
    from(clientBuildDir)
    into("src/jvmMain/resources/static")
}

tasks.named("jvmProcessResources") {
    dependsOn("copyClientToServer")
}
