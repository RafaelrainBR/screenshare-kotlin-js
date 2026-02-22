# Domain: multiplatform-common

## Overview
The `common` Gradle module is a Kotlin Multiplatform library targeting both JVM and JS. It is the sole location for code shared between the server and the client. It contains only the packet protocol and data models.

---

## Files
- `common/src/commonMain/kotlin/screenshare/common/Packet.kt`
- `common/src/commonMain/kotlin/screenshare/common/ChatMessage.kt`
- `common/src/commonTest/kotlin/screenshare/common/MessageSpec.kt`
- `common/build.gradle.kts`

---

## Build Configuration

```kotlin
// common/build.gradle.kts
plugins {
    alias(libs.plugins.kotlinMultiplatform)
    alias(libs.plugins.ksp)
    alias(libs.plugins.kotlinSerialization)
    alias(libs.plugins.kotestMultiplatform)
}

kotlin {
    js { nodejs() }
    jvm()

    sourceSets {
        commonMain.dependencies {
            implementation(libs.kotlinx.serialization.json)
        }
        commonTest.dependencies {
            implementation(libs.kotest.assertions.core)
            implementation(libs.kotest.framework.engine)
        }
        jvmTest.dependencies {
            implementation(libs.kotest.runner.junit5)
        }
    }
}
```

---

## ChatMessage

Simple serializable data class:
```kotlin
@Serializable
data class ChatMessage(
    val username: String,
    val content: String,
    val timestamp: Long,
)
```

---

## SocketUser

Represents a participant as seen by other users:
```kotlin
@Serializable
data class SocketUser(
    val socketId: String,
    val username: String,
    val roomId: String,
    val isMuted: Boolean = true,
)
```

---

## Packet Serialization Rules

1. Every packet subclass must have `@Serializable` and `@SerialName("<kebab-case-type>")`.
2. Fields use short `@SerialName` aliases (see table in `signaling-layer.md`).
3. The JSON discriminator key is `"type"` (default for Kotlin sealed class serialization).

### Serialization Examples
```kotlin
// Serialize
Json.encodeToString<Packet>(JoinRoom("room1", "Alice"))
// → {"type":"join-room","rid":"room1","username":"Alice"}

// Deserialize
Json.decodeFromString<Packet>("""{"type":"join-room","rid":"room1","username":"Alice"}""")
// → JoinRoom(roomId="room1", username="Alice")
```

---

## Tests (MessageSpec.kt)

Tests use Kotest's `FunSpec` + `withData` for parameterized serialization round-trips:
```kotlin
class MessageSpec : FunSpec({
    withData(
        nameFn = { "should deserialize json into Message [${it.second}]" },
        joinRoomJson("testRoomId", "username") to JoinRoom("testRoomId", "username"),
        // ...
    ) { (json, expected) ->
        Json.decodeFromString<Packet>(json) shouldBe expected
    }
})
```

Serialized JSON strings are built by private companion helper functions (e.g., `joinRoomJson(roomId, username)`).

---

## Constraints
- No JVM-only or JS-only APIs allowed in `commonMain`
- Only `kotlinx.serialization` and the Kotlin standard library may be used
- New data classes must be `@Serializable` to cross the wire
