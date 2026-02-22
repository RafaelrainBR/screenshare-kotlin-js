# Style Guide: utility-functions

## Unique Conventions

### Top-Level Functions, No Class Wrapper
Utility functions are top-level Kotlin functions in `Util.kt`, not wrapped in an object or class.

### String Extensions
Utility functions on `String` are defined as extension functions:
```kotlin
fun String.getUsernameInitials(): String =
    this.split(" ")
        .mapNotNull { it.firstOrNull()?.toString()?.uppercase() }
        .take(2)
        .joinToString("")

fun String.sanitizeHTML(): String =
    this
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&#39;")
```

### UUID Room IDs via kotlin.uuid
Random room IDs are generated using the experimental `kotlin.uuid.Uuid` API:
```kotlin
@OptIn(ExperimentalUuidApi::class)
fun generateRandomRoomId(): String = Uuid.random().toString()
```

Callers trim to 8 characters: `generateRandomRoomId().take(8)`.

### Document Extension for Safe Element Creation
```kotlin
fun Document.createSafeElement(tag: String): Element = this.createElement(tag)
```

Used in `UserListMutations` to create DOM elements with a consistent API.
