# Style Guide: ui-dom-access

## Unique Conventions

### Singleton Object for All References
All DOM element bindings live in `object Elements`. No individual `document.getElementById()` calls appear in business logic files.

```kotlin
object Elements {
    val joinScreen = getElement<HTMLElement>("join-screen")
    val appScreen  = getElement<HTMLElement>("app-screen")
    // ...
}
```

### Typed Generic Helper
A private inline function ensures compile-time type safety and a clear error message:
```kotlin
private inline fun <reified T : HTMLElement> getElement(id: String): T =
    runCatching { document.getElementById(id) as T }.getOrElse {
        throw IllegalStateException("Element with id '$id' not found or is not of type ${T::class.simpleName}")
    }
```

### Matching HTML IDs Exactly
Every string passed to `getElement(id)` must match an `id` attribute in `index.html` exactly. The convention is `kebab-case` IDs (e.g., `"join-screen"`, `"chat-messages"`, `"shareScreenBtn"`).

### Property Naming
Properties in `Elements` use `camelCase` and are named after what the element _is_, not its HTML ID verbatim (e.g., `joinScreen` for `id="join-screen"`, `stopScreenShareButton` for `id="stopSharingBtn"`).
