# Style Guide: tests

## Unique Conventions

### Kotest FunSpec + withData
Tests use Kotest's `FunSpec` style with `withData` for parameterized cases:
```kotlin
class MessageSpec : FunSpec({
    withData(
        nameFn = { "should deserialize json into Message [${it.second}]" },
        jsonString to expectedPacket,
        // ...
    ) { (json, expected) ->
        Json.decodeFromString<Packet>(json) shouldBe expected
    }
})
```

### Companion Object Helpers for JSON Strings
Expected JSON strings are built by private `companion object` functions, not inline. This makes the test cases readable and consistent:
```kotlin
private companion object {
    fun joinRoomJson(roomId: String, username: String) =
        """{"type":"join-room","rid":"$roomId","username":"$username"}"""
    fun sendMessageJson(roomId: String, message: String) =
        """{"type":"send-message","rid":"$roomId","msg":"$message"}"""
}
```

### Pair Syntax for Test Data
Test data is expressed as `json to expectedObject` pairs passed to `withData`.

### shouldBe Assertions
The only assertion style used is `shouldBe` from `kotest-assertions-core`.

### Test Naming Convention
Class names end in `Spec` (e.g., `MessageSpec`). Method names are descriptive sentences: `"should deserialize json into Message [...]"`.

### Multiplatform Test Runs
Tests in `commonTest` run on both JS (Node.js) and JVM. The `kotestMultiplatform` plugin enables this. JVM needs `kotest-runner-junit5` in `jvmTest`.
