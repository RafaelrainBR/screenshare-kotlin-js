# Style Guide: ui-mutations

## Unique Conventions

### Mutation Objects (Singleton Pattern)
DOM modifications are grouped in `object` singletons: `InterfaceMutations` (global UI) and `UserListMutations` (user list panel). Direct DOM calls outside these objects are avoided.

### Show/Hide via `classList.add/remove("hidden")`
Visibility toggles always use the DaisyUI/Tailwind `hidden` class:
```kotlin
Elements.videoContainer.classList.remove("hidden")
Elements.noScreenMessage.classList.add("hidden")
```

### Dynamic Element Creation with `document.createElement`
New chat messages and user list items are created programmatically:
```kotlin
val messageElement = document.createElement("div") as HTMLDivElement
messageElement.className = "animate-fade-in"
Elements.chatMessages.appendChild(messageElement)
messageElement.scrollIntoView(js("{ behavior: 'smooth', block: 'end' }"))
```

### DaisyUI Component Classes in Kotlin Strings
DaisyUI class strings (`"chat"`, `"chat-end"`, `"chat-bubble"`, `"chat-bubble-primary"`, `"avatar"`, `"placeholder"`, `"ring-2"`, `"ring-success"`) are applied as raw className strings.

### Conditional Class Naming
User differentiation uses Kotlin conditional expressions:
```kotlin
wrapper.className = "chat ${if (isCurrentUser) "chat-end" else "chat-start"}"
```

### Avatar Initials via Utility Function
User avatars display initials via `String.getUsernameInitials()` from `Util.kt`:
```kotlin
val initials = user.username.getUsernameInitials()
// "Alice" → "A", "Maria Silva" → "MS"
```

### Brazilian Portuguese UI Strings
All user-facing strings are in pt-BR:
- `"Sistema"` for system messages
- `"Você entrou na sala $roomId"`, `"${username} entrou na sala"`, `"${username} saiu da sala"`
- `"Conexão encerrada! Recarregue a página."`
- `"Você"` for own messages in chat

### UserListMutations: State via CSS Classes
Speaking indicator: adds/removes `"ring-2 ring-success ring-offset-2 ring-offset-base-100"` on `".avatar-circle"`.
Mute indicator: toggles `"text-error"` / `"text-success"` / `"hidden"` on `#micIcon-user-{socketId}`.
