# Plano: Módulo `client-kmp` — Compose Multiplatform (Desktop + Web)

> **Data**: 2026-02-22
> **Status**: Draft
> **Escopo inicial**: WebRTC

## TL;DR

Criar `client-kmp/`, um novo módulo KMP coexistindo com `client/`. Usa **Compose Multiplatform 1.10.1** para UI
compartilhada entre **Desktop (JVM)** e **Web (wasmJs)**. WebRTC via fork própria do
[webrtc-kmp](https://github.com/shepeliev/webrtc-kmp) com suporte JVM baseado nos
[commits de aschulz90](https://github.com/aschulz90/webrtc-kmp). A sinalização (Ktor WebSocket)
e o protocolo (`Packet`) são reusados integralmente do módulo `common/` existente. O client JS
atual (`client/`) continua funcionando sem alterações.

---

## Contexto Técnico Verificado

| Item                                | Valor atual do projeto | Observação                                                       |
|-------------------------------------|------------------------|------------------------------------------------------------------|
| Kotlin                              | 2.3.10                 | Compose 1.10.1 é compatível (requer ≥2.1.0)                     |
| Compose Multiplatform (alvo)        | 1.10.1 (estável)       | Última estável em fev/2026. Web entry: `ComposeViewport{}`       |
| Ktor                                | 3.4.0                  | Suporta cliente wasmJs e JVM                                     |
| kotlinx-serialization               | 1.10.0                 | Suporta wasmJs                                                   |
| webrtc-kmp upstream (shepeliev)     | Kotlin 2.1.21          | Sem JVM. Mais atualizado que a fork                              |
| webrtc-kmp fork (aschulz90)         | Kotlin 2.0.21          | Com JVM, mas **2 anos desatualizada**. webrtc-java 0.8.0         |
| webrtc-java (devopvoid)             | 0.14.0                 | Windows/macOS/Linux. Screen capture + audio + peer connections   |
| `common/` (Packet.kt, ChatMessage)  | js + jvm               | Precisa adicionar target `wasmJs`                                |

### Riscos e Decisões

| Risco                                                           | Mitigação                                                                                  |
|-----------------------------------------------------------------|--------------------------------------------------------------------------------------------|
| Fork webrtc-kmp está 2+ anos desatualizada (Kotlin 2.0.21)     | Fazer fork próprio do upstream (2.1.21), aplicar commits JVM, atualizar para Kotlin 2.3.10 |
| webrtc-java 0.8.0 → 0.14.0 pode ter breaking changes           | Atualizar junto com o fork; APIs core (PeerConnection) são estáveis                        |
| WebRTC JVM no macOS tem problema de permissões de áudio/câmera | Suportar apenas Windows inicialmente; câmera não é usada                                   |
| Compose for Web (wasmJs) requer browsers com WasmGC             | Alternativa: adicionar target `js` para retrocompatibilidade via compatibility mode         |
| `CanvasBasedWindow` foi removido no CMP 1.11                    | Usar `ComposeViewport{}` (API estável no 1.10.1) no source set `webMain`                   |
| Audio level monitor usa Web Audio API (JS-only)                 | Implementar via `expect/actual`; JVM terá stub sem indicador visual de "falando"            |

---

## Estrutura de Módulos (Pós-Implementação)

```
screenshare/
├── common/                   # Protocolo (Packet, ChatMessage) — js, jvm, wasmJs
├── client/                   # Client JS atual (inalterado)
├── client-kmp/               # ★ NOVO — Compose Multiplatform
│   ├── build.gradle.kts
│   └── src/
│       ├── commonMain/       # AppState, Session, Websocket, SessionHandler, UI Compose
│       ├── jvmMain/          # Entry point Desktop, HttpClient engine
│       ├── wasmJsMain/       # (vazio ou overrides pontuais)
│       └── webMain/          # Entry point Web (ComposeViewport)
├── libs/
│   └── webrtc-kmp/           # ★ Git submodule — fork próprio com JVM support
├── server/
├── server-java/
└── ...
```

---

## Fases de Implementação

### Fase 0 — Preparar dependência webrtc-kmp

**0.1 — Criar fork próprio do upstream `shepeliev/webrtc-kmp`**

Fazer fork no GitHub a partir do upstream (Kotlin 2.1.21). Aplicar os commits JVM do
branch `main` de `aschulz90/webrtc-kmp`. Os arquivos JVM são ~20 arquivos em
`webrtc-kmp/src/jvmMain/kotlin/com/shepeliev/webrtckmp/`:

- `PeerConnection.kt`, `MediaDevices.kt`, `MediaStream.kt`, `MediaStreamTrackImpl.kt`
- `DataChannel.kt`, `DtmfSender.kt`, `IceCandidate.kt`, `IceServer.kt`
- `RtcConfiguration.kt`, `RtcCertificatePem.kt`, `RtcStats.kt`, `RtcStatsReport.kt`
- `RtpParameters.kt`, `RtpReceiver.kt`, `RtpSender.kt`, `RtpTransceiver.kt`
- `SessionDescriptionExt.kt`, `PeerConnectionExt.kt`
- `LocalAudioStreamTrack.kt`, `LocalVideoStreamTrack.kt`, `RemoteAudioStreamTrack.kt`,
  `RemoteVideoStreamTrack.kt`, `DesktopVideoStreamTrack.kt`, `RenderedVideoStreamTrack.kt`,
  `VideoStreamTrack.kt`
- `WebRtc.kt`, `ByteBuffer.kt`

**0.2 — Atualizar o fork**

- Kotlin: 2.1.21 → 2.3.10
- webrtc-java: 0.8.0 → 0.14.0
- kotlinx-coroutines: atualizar para 1.10.2
- Remover targets desnecessários para o MVP (iOS, Android) se causar problemas de build — ou manter
- Testar: `./gradlew :webrtc-kmp:jvmTest` com `windows-x86_64`

**0.3 — Adicionar como Git submodule + composite build**

```bash
git submodule add https://github.com/<seu-usuario>/webrtc-kmp.git libs/webrtc-kmp
```

Em `settings.gradle.kts`:

```kotlin
includeBuild("libs/webrtc-kmp") {
    dependencySubstitution {
        substitute(module("com.shepeliev:webrtc-kmp")).using(project(":webrtc-kmp"))
    }
}
```

> **Nota**: esta fase pode ser postergada para depois do MVP (chat-only). O MVP não precisa de
> WebRTC. A fase 0 é necessária apenas quando WebRTC for integrado (Fase 5+).

---

### Fase 1 — Criar módulo `client-kmp` e configurar build

**1.1 — Versões e plugins em `libs.versions.toml`**

Adicionar:

```toml
[versions]
compose-multiplatform = "1.10.1"
kotlinx-datetime = "0.6.2"

[libraries]
kotlinx-datetime = { group = "org.jetbrains.kotlinx", name = "kotlinx-datetime", version.ref = "kotlinx-datetime" }
ktor-client-okhttp = { group = "io.ktor", name = "ktor-client-okhttp", version.ref = "ktor" }
ktor-client-js = { group = "io.ktor", name = "ktor-client-js", version.ref = "ktor" }

[plugins]
compose-multiplatform = { id = "org.jetbrains.compose", version.ref = "compose-multiplatform" }
compose-compiler = { id = "org.jetbrains.kotlin.plugin.compose", version.ref = "kotlin" }
```

**1.2 — Registrar módulo em `settings.gradle.kts`**

```kotlin
include(":client-kmp")
```

**1.3 — Adicionar target `wasmJs` ao `common/build.gradle.kts`**

O módulo `common/` hoje tem targets `js { nodejs() }` e `jvm()`. Adicionar `wasmJs()` para que
`client-kmp` possa depender dele no target wasmJs:

```kotlin
kotlin {
    js {
        nodejs()
    }
    jvm()
    wasmJs {           // ★ NOVO
        browser()
    }
    // sourceSets inalterados
}
```

> **Impacto**: nenhum para `client/` e `server/`. O `wasmJs` target só gera artefatos extras.
> Verificar se `kotlinx-serialization-json` e `kotest` suportam `wasmJs` — sim, ambos suportam.

**1.4 — Criar `client-kmp/build.gradle.kts`**

```kotlin
import org.jetbrains.compose.desktop.application.dsl.TargetFormat

plugins {
    alias(libs.plugins.kotlinMultiplatform)
    alias(libs.plugins.kotlinSerialization)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.compose.compiler)
}

kotlin {
    jvm("desktop")

    wasmJs {
        browser()
        binaries.executable()
    }

    sourceSets {
        val desktopMain by getting

        commonMain.dependencies {
            implementation(project(":common"))

            implementation(compose.runtime)
            implementation(compose.foundation)
            implementation(compose.material3)
            implementation(compose.ui)
            implementation(compose.components.resources)

            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.kotlinx.serialization.json)

            implementation(libs.ktor.client.core)
            implementation(libs.ktor.client.websockets)
            implementation(libs.ktor.serialization.kotlinx.json)
        }

        desktopMain.dependencies {
            implementation(compose.desktop)
            implementation(libs.kotlinx.coroutines.swing)
            implementation(libs.ktor.client.okhttp)
        }

        wasmJsMain.dependencies {
            // Ktor auto-configura engine para wasmJs
        }
    }
}

compose.desktop {
    application {
        mainClass = "MainKt"

        nativeDistributions {
            targetFormats(TargetFormat.Msi, TargetFormat.Deb)
            packageName = "screenshare-desktop"
            packageVersion = "1.0.0"
        }
    }
}
```

> **Dependências novas necessárias em `libs.versions.toml`** (além das já adicionadas em 1.1):
>
> ```toml
> [libraries]
> kotlinx-coroutines-swing = { group = "org.jetbrains.kotlinx", name = "kotlinx-coroutines-swing", version.ref = "kotlinx-coroutines" }
> ```

**1.5 — Verificar build vazio**

```bash
./gradlew :client-kmp:desktopMainClasses :client-kmp:wasmJsBrowserDevelopmentRun --dry-run
```

---

### Fase 2 — Estado e lógica de sessão (`commonMain`)

A migração das classes do `client/` (JS-only) para `client-kmp/commonMain` requer eliminar toda
dependência de APIs JS (`dynamic`, `kotlin.js.Json`, `window`, `document`, DOM, Web APIs).
A lógica pura (protocolo, estado, WebSocket dispatch) pode ser portada diretamente.

**Mapeamento JS → KMP:**

| JS (`client/`)                          | KMP (`client-kmp/commonMain`)                    | Notas                                          |
|-----------------------------------------|--------------------------------------------------|-------------------------------------------------|
| `var session: Session?` (global)        | `AppState` (Compose `StateFlow`)                 | State hoisting via `MutableStateFlow`           |
| `Session` (class, `CoroutineScope by`)  | `SessionManager` (class, `CoroutineScope by`)    | Sem mutabilidade exposta; emite via `StateFlow` |
| `WebsocketService` (JS HttpClient)      | `WebsocketService` (Ktor multiplatform client)   | Remove `kotlin.js.Json`; usa `String` para ICE  |
| `handlePacket()` (top-level fun)        | `PacketHandler` (class que recebe `AppState`)    | Sem `console`, usa `println` ou logging KMP     |
| `InterfaceMutations` / `Elements`       | Composables (reativos via State)                 | UI inteiramente recriada em Compose             |
| `PeerConnections` (JS `dynamic`)        | (Fase 5 — via webrtc-kmp)                        | Inicialmente stub                               |
| `VoiceChat` / `ScreenSharing`           | (Fase 5 — via expect/actual + webrtc-kmp)        | Inicialmente stub                               |

**2.1 — `AppState.kt` — Estado reativo da aplicação**

```
client-kmp/src/commonMain/kotlin/screenshare/clientkmp/state/AppState.kt
```

```kotlin
package screenshare.clientkmp.state

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import screenshare.common.ChatMessage
import screenshare.common.SocketUser

data class RoomState(
    val roomId: String,
    val username: String,
    val users: List<SocketUser> = emptyList(),
    val chatMessages: List<ChatMessage> = emptyList(),
    val currentSharerSocketId: String? = null,
    val isMicMuted: Boolean = true,
    val speakingUsers: Set<String> = emptySet(),
)

sealed class Screen {
    data object Join : Screen()
    data class Room(val state: RoomState) : Screen()
}

class AppState {
    private val _screen = MutableStateFlow<Screen>(Screen.Join)
    val screen: StateFlow<Screen> = _screen.asStateFlow()

    fun navigateToRoom(roomId: String, username: String) {
        _screen.value = Screen.Room(RoomState(roomId = roomId, username = username))
    }

    fun updateRoom(transform: (RoomState) -> RoomState) {
        val current = _screen.value
        if (current is Screen.Room) {
            _screen.value = Screen.Room(transform(current.state))
        }
    }

    val currentRoom: RoomState?
        get() = (_screen.value as? Screen.Room)?.state
}
```

**2.2 — `WebsocketService.kt` — WebSocket multiplataforma**

```
client-kmp/src/commonMain/kotlin/screenshare/clientkmp/services/WebsocketService.kt
```

Portar de `client/services/WebsocketService.kt` removendo:
- `kotlin.js.Json` → usar `String` para ICE candidates
- `getSessionOrAlert()` (global mutable) → receber `AppState` via construtor
- Engine injection via `expect/actual` ou Ktor auto-detect

Assinatura:

```kotlin
class WebsocketService(
    private val urlProtocol: URLProtocol,
    private val host: String,
    private val port: Int,
    private val onPacketReceived: suspend (Packet) -> Unit,
    private val onClose: () -> Unit = {},
) {
    suspend fun connect()
    suspend fun joinRoom(roomId: String, username: String)
    suspend fun sendChatMessage(roomId: String, message: String)
    suspend fun sendToggleMute(roomId: String, isMuted: Boolean)
    suspend fun sendIceCandidate(roomId: String, candidate: String, targetId: String)
    suspend fun sendDescription(roomId: String, description: Map<String, String>, targetId: String)
    suspend fun startScreenSharing(roomId: String)
    suspend fun stopScreenSharing(roomId: String)
}
```

> **Engine**: `desktopMain` usa `OkHttp`; `wasmJsMain` usa engine automático do Ktor para wasmJs.

**2.3 — `PacketHandler.kt` — Dispatch de pacotes recebidos**

```
client-kmp/src/commonMain/kotlin/screenshare/clientkmp/services/PacketHandler.kt
```

Portar de `client/services/SessionHandler.kt`. Principais mudanças:
- Recebe `AppState` em vez de `Session`
- Atualiza estado via `appState.updateRoom { ... }` (reativo, Compose recompõe automaticamente)
- Mensagens de sistema usam `username = "Sistema"` (mantém padrão)
- WebRTC-related packets (`IceCandidateReceived`, `DescriptionReceived`) delegam para `WebRtcManager`
  (stub na Fase 2, implementado na Fase 5)

```kotlin
class PacketHandler(
    private val appState: AppState,
    private val webRtcManager: WebRtcManager?, // null até Fase 5
) {
    suspend fun handle(packet: Packet) {
        when (packet) {
            is Packet.UserConnected -> handleUserConnected(packet)
            is Packet.UserDisconnected -> handleUserDisconnected(packet)
            is Packet.ChatMessageReceived -> handleChatMessage(packet)
            is Packet.UserList -> handleUserList(packet)
            is Packet.IceCandidateReceived -> webRtcManager?.handleIceCandidate(packet)
            is Packet.DescriptionReceived -> webRtcManager?.handleDescription(packet)
            is Packet.UserMuted -> handleMuteChange(packet.socketId, isMuted = true)
            is Packet.UserUnmuted -> handleMuteChange(packet.socketId, isMuted = false)
            is Packet.ScreenShareStarted -> {}
            is Packet.ScreenShareStopped -> {}
            else -> println("Unknown packet: ${packet::class.simpleName}")
        }
    }
}
```

**2.4 — `SessionManager.kt` — Orquestrador de sessão**

```
client-kmp/src/commonMain/kotlin/screenshare/clientkmp/services/SessionManager.kt
```

Substitui `Session` do client JS. Coordena `WebsocketService`, `PacketHandler` e `AppState`.

```kotlin
class SessionManager(
    private val appState: AppState,
    private val websocketService: WebsocketService,
    private val coroutineScope: CoroutineScope,
) {
    fun joinRoom(username: String, roomId: String) {
        coroutineScope.launch {
            appState.navigateToRoom(roomId, username)
            websocketService.joinRoom(roomId, username)
            appState.updateRoom { it.copy(
                chatMessages = it.chatMessages + systemMessage("Você entrou na sala $roomId")
            )}
        }
    }

    fun sendMessage(message: String) {
        val room = appState.currentRoom ?: return
        coroutineScope.launch {
            websocketService.sendChatMessage(room.roomId, message)
        }
    }

    fun toggleMic() {
        val room = appState.currentRoom ?: return
        coroutineScope.launch {
            val newMuted = !room.isMicMuted
            appState.updateRoom { it.copy(isMicMuted = newMuted) }
            websocketService.sendToggleMute(room.roomId, newMuted)
            // WebRTC mic toggle — Fase 5
        }
    }

    fun startScreenShare() { /* Fase 5 */ }
    fun stopScreenShare()  { /* Fase 5 */ }
}
```

---

### Fase 3 — UI Compose (`commonMain`)

Toda a UI é construída em `commonMain` usando Compose Multiplatform Material 3.
Substitui integralmente `index.html` + `Elements.kt` + `InterfaceMutations.kt` +
`UserListMutations.kt` + `InterfaceHandlers.kt`.

**Estrutura de arquivos UI:**

```
client-kmp/src/commonMain/kotlin/screenshare/clientkmp/ui/
├── App.kt                    # Root composable + navigation
├── theme/
│   └── Theme.kt              # Material 3 dark theme (equivale ao night DaisyUI)
├── screens/
│   ├── JoinScreen.kt         # Tela de entrada (username + roomId)
│   └── RoomScreen.kt         # Tela principal da sala
├── components/
│   ├── ChatPanel.kt          # Painel de chat (mensagens + input)
│   ├── ChatBubble.kt         # Bolha individual de mensagem
│   ├── UserListPanel.kt      # Lista de usuários online
│   ├── UserListItem.kt       # Item individual do user list
│   ├── ScreenShareArea.kt    # Área de vídeo (placeholder até Fase 5)
│   └── Toolbar.kt            # Barra de ações (mic, screen share, fullscreen)
```

**3.1 — `Theme.kt` — Tema escuro Material 3**

Mapeamento DaisyUI `night` → Material 3:

| DaisyUI (night)                     | Material 3                                    |
|-------------------------------------|-----------------------------------------------|
| `--b1` (base-100) `#0f1729`        | `surface` / `background`                      |
| `--b2` (base-200)                   | `surfaceVariant`                               |
| `--b3` (base-300)                   | `surfaceContainerHighest`                      |
| `--p` (primary) `#38bdf8`          | `primary`                                     |
| `--pc` (primary-content)            | `onPrimary`                                   |
| `--s` (secondary) `#818cf8`        | `secondary`                                   |
| `--er` (error) `#fb7185`           | `error`                                       |
| `--su` (success) `#2dd4bf`         | `tertiary` _(usado para "online"/"speaking")_ |
| `chat-bubble-primary`               | `primaryContainer` + `onPrimaryContainer`      |
| `bg-base-300` (other bubble)        | `surfaceVariant` + `onSurfaceVariant`          |

```kotlin
@Composable
fun ScreenShareTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = Color(0xFF38BDF8),
            onPrimary = Color(0xFF00344E),
            primaryContainer = Color(0xFF004B6F),
            secondary = Color(0xFF818CF8),
            surface = Color(0xFF0F1729),
            background = Color(0xFF0F1729),
            error = Color(0xFFFB7185),
            // ...
        ),
        content = content,
    )
}
```

**3.2 — `App.kt` — Root composable**

```kotlin
@Composable
fun App(sessionManager: SessionManager, appState: AppState) {
    val screen by appState.screen.collectAsState()

    ScreenShareTheme {
        Surface(modifier = Modifier.fillMaxSize()) {
            when (val s = screen) {
                is Screen.Join -> JoinScreen(
                    onJoin = { username, roomId ->
                        sessionManager.joinRoom(username, roomId)
                    }
                )
                is Screen.Room -> RoomScreen(
                    state = s.state,
                    onSendMessage = { sessionManager.sendMessage(it) },
                    onToggleMic = { sessionManager.toggleMic() },
                    onStartScreenShare = { sessionManager.startScreenShare() },
                    onStopScreenShare = { sessionManager.stopScreenShare() },
                )
            }
        }
    }
}
```

**3.3 — `JoinScreen.kt`**

Dois campos (`username`, `roomId`) + botão "Entrar". `roomId` gera valor aleatório se vazio
(usa `Uuid.random()` como no `Util.kt` atual). Textos em pt-BR.

**3.4 — `RoomScreen.kt`**

Layout principal — divide em:
- **Coluna esquerda (70%)**: `ScreenShareArea` (topo) + `ChatPanel` (baixo)
- **Coluna direita (30%)**: `UserListPanel` + `Toolbar`

Em telas menores (< 800dp), altera para layout vertical com tabs.

**3.5 — `ChatPanel.kt` + `ChatBubble.kt`**

- `LazyColumn` com items = `state.chatMessages`
- Cada `ChatBubble` renderiza:
  - Avatar com iniciais (`getUsernameInitials()` — copiar de `Util.kt`)
  - Header: nome + timestamp
  - Bolha: `primaryContainer` para mensagens próprias, `surfaceVariant` para outras
  - Alinhamento: `Arrangement.End` para próprias, `Start` para outras
- Input row: `OutlinedTextField` + `IconButton(send)`
- Auto-scroll via `LaunchedEffect(messages.size)` + `scrollToItem(lastIndex)`

**3.6 — `UserListPanel.kt` + `UserListItem.kt`**

- `LazyColumn` com items = `state.users`
- Cada item mostra:
  - Avatar circular com iniciais
  - Nome (+ " (Você)" se local)
  - Ícone de mic (vermelho/slash quando muted)
  - Ring verde quando `socketId in state.speakingUsers`
- Header: "Participantes (N)"

**3.7 — `ScreenShareArea.kt`**

- Fase 2-4: placeholder com ícone + texto "Nenhuma tela sendo compartilhada"
- Fase 5: integra vídeo via webrtc-kmp `VideoRenderer` composable

**3.8 — `Toolbar.kt`**

Row com `IconButton`s:
- 🎤 Mic toggle (estado baseado em `state.isMicMuted`)
- 🖥 Share Screen / Stop Sharing
- ⛶ Fullscreen (desktop only — `expect/actual` ou conditionally shown)

---

### Fase 4 — Entry points (platform-specific)

**4.1 — Desktop (JVM) — `desktopMain`**

```
client-kmp/src/desktopMain/kotlin/Main.kt
```

```kotlin
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application

fun main() = application {
    val appState = remember { AppState() }
    val coroutineScope = rememberCoroutineScope()

    val websocketService = remember {
        WebsocketService(
            urlProtocol = URLProtocol.WS,
            host = "localhost",    // configurável via args ou UI
            port = 8080,
            onPacketReceived = { packet ->
                packetHandler.handle(packet)
            },
        )
    }

    val packetHandler = remember { PacketHandler(appState, webRtcManager = null) }
    val sessionManager = remember { SessionManager(appState, websocketService, coroutineScope) }

    LaunchedEffect(Unit) {
        websocketService.connect()
    }

    Window(
        onCloseRequest = ::exitApplication,
        title = "ScreenShare",
    ) {
        App(sessionManager, appState)
    }
}
```

> **Nota**: Host/porta do servidor podem ser configurados via:
> 1. Argumentos da linha de comando (`args`)
> 2. Campo na `JoinScreen` (adicionar campo "Server URL")
> 3. Arquivo de configuração `~/.screenshare/config.properties`

**4.2 — Web (wasmJs) — `wasmJsMain`**

```
client-kmp/src/wasmJsMain/kotlin/Main.kt
```

```kotlin
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.window.ComposeViewport
import kotlinx.browser.document
import kotlinx.browser.window

@OptIn(ExperimentalComposeUiApi::class)
fun main() {
    ComposeViewport(document.body!!) {
        val appState = remember { AppState() }
        val coroutineScope = rememberCoroutineScope()

        val websocketService = remember {
            val loc = window.location
            WebsocketService(
                urlProtocol = if (loc.protocol == "https:") URLProtocol.WSS else URLProtocol.WS,
                host = loc.hostname,
                port = loc.port.toIntOrNull() ?: if (loc.protocol == "https:") 443 else 80,
                onPacketReceived = { /* ... */ },
            )
        }

        val packetHandler = remember { PacketHandler(appState, webRtcManager = null) }
        val sessionManager = remember { SessionManager(appState, websocketService, coroutineScope) }

        LaunchedEffect(Unit) { websocketService.connect() }

        App(sessionManager, appState)
    }
}
```

**4.3 — Verificação E2E (Fase 4)**

```bash
# Desktop
./gradlew :client-kmp:run

# Web (wasmJs dev server)
./gradlew :client-kmp:wasmJsBrowserDevelopmentRun
```

Ambos devem:
1. Mostrar `JoinScreen`
2. Conectar ao WebSocket do servidor (`./gradlew :server-java:run`)
3. Entrar na sala → ver `RoomScreen`
4. Enviar/receber mensagens de chat
5. Ver lista de usuários atualizar em tempo real

---

### Fase 5 — Integração WebRTC

> **Pré-requisito**: Fase 0 concluída (fork webrtc-kmp com JVM support atualizado)

**5.1 — Adicionar dependência webrtc-kmp**

Em `client-kmp/build.gradle.kts`:

```kotlin
commonMain.dependencies {
    implementation("com.shepeliev:webrtc-kmp:$webrtcKmpVersion")
    // via composite build, resolve para o submodule local
}
```

**5.2 — `WebRtcManager.kt` — Gerenciador de peer connections**

```
client-kmp/src/commonMain/kotlin/screenshare/clientkmp/services/WebRtcManager.kt
```

Portar lógica de `PeerConnections.kt` usando APIs do webrtc-kmp:

| JS (`client/`)                            | webrtc-kmp (`client-kmp/`)                         |
|-------------------------------------------|-----------------------------------------------------|
| `RTCPeerConnectionDecorator` (dynamic)    | `PeerConnection()` (webrtc-kmp typed API)           |
| `peerConnection.createOffer().await()`    | `peerConnection.createOffer()`  (suspend)           |
| `peerConnection.setLocalDescription()`    | `peerConnection.setLocalDescription()`              |
| `JSON.parse<Json>(candidate)`             | `IceCandidate(sdp, sdpMid, sdpMLineIndex)`          |
| `peerConnection.addTrack(track, stream)`  | `peerConnection.addTrack(track, stream)`            |
| `peerConnection.ontrack` (dynamic)        | `peerConnection.onTrack` (Flow/callback)            |

Estrutura:

```kotlin
class WebRtcManager(
    private val appState: AppState,
    private val websocketService: WebsocketService,
    private val coroutineScope: CoroutineScope,
) {
    private val peers = mutableMapOf<String, PeerConnection>()

    fun createPeerConnection(socketId: String, isInitiator: Boolean) { ... }
    fun closePeerConnection(socketId: String) { ... }
    suspend fun handleIceCandidate(packet: Packet.IceCandidateReceived) { ... }
    suspend fun handleDescription(packet: Packet.DescriptionReceived) { ... }
    fun recreateAllConnections() { ... }
}
```

**5.3 — Media Capture — `expect/actual`**

```
client-kmp/src/commonMain/kotlin/.../media/MediaCapture.kt     (expect)
client-kmp/src/desktopMain/kotlin/.../media/MediaCapture.kt    (actual — webrtc-java)
client-kmp/src/wasmJsMain/kotlin/.../media/MediaCapture.kt     (actual — browser APIs via webrtc-kmp)
```

```kotlin
// commonMain — expect
expect class MediaCapture {
    suspend fun getDisplayMedia(): MediaStream
    suspend fun getUserMedia(): MediaStream
}
```

- **Desktop (JVM)**: usa webrtc-java `MediaDevices.getDisplayMedia()` para screen capture,
  `MediaDevices.getUserMedia()` para mic
- **wasmJs**: usa webrtc-kmp que delega para browser `navigator.mediaDevices`

**5.4 — Audio Level Monitor — `expect/actual`**

```kotlin
// commonMain
expect fun monitorAudioLevel(
    stream: MediaStream,
    onSpeakingChange: (Boolean) -> Unit,
): Closeable

// wasmJsMain — usa Web Audio API (AudioContext + AnalyserNode) como no VoiceChat.kt atual
// desktopMain — stub que nunca dispara isSpeaking (ou usa javax.sound se viável)
```

**5.5 — Video Rendering no Compose**

- **Desktop**: webrtc-kmp fornece `VideoRenderer` composable (ou usar `SwingPanel` com canvas)
- **wasmJs**: webrtc-kmp fornece `Video` composable que renderiza `<video>` element

Substituir o placeholder de `ScreenShareArea.kt` pelo renderer real.

---

### Fase 6 — Polimento e paridade

**6.1 — URL query params**

- Web: ler `?roomId=xxx` da URL para preencher campo (como no client JS atual)
- Desktop: aceitar `--room=xxx --server=wss://host` via args

**6.2 — Reconexão automática**

O client JS mostra alert e pede reload. Para Desktop, implementar reconnect com backoff.

**6.3 — Notificação de disconnection**

Mensagem de sistema "Conexão encerrada" no chat (já no `AppState`, atualizar via `PacketHandler`
ou `onClose` callback).

**6.4 — Testes**

| Tipo                          | Onde                                    | Framework       |
|-------------------------------|-----------------------------------------|-----------------|
| Serialização de Packet        | `common/commonTest` (existentes)        | Kotest FunSpec  |
| AppState unit tests           | `client-kmp/commonTest`                 | Kotest FunSpec  |
| PacketHandler unit tests      | `client-kmp/commonTest`                 | Kotest FunSpec  |
| SessionManager unit tests     | `client-kmp/commonTest`                 | Kotest FunSpec  |
| Compose UI snapshot tests     | `client-kmp/desktopTest`                | Compose Preview  |

**6.5 — Build e distribuição**

```bash
# Desktop distributable
./gradlew :client-kmp:packageMsi          # Windows
./gradlew :client-kmp:packageDeb          # Linux

# Web production build
./gradlew :client-kmp:wasmJsBrowserDistribution
```

Para embutir o client wasmJs no server (como o client JS atual), adicionar task análoga ao
`copyClientToServer` existente:

```kotlin
// server/build.gradle.kts
val clientKmpBuildDir = project(":client-kmp").layout.buildDirectory.dir("dist/wasmJs/productionExecutable")

tasks.register<Copy>("copyClientKmpToServer") {
    dependsOn(":client-kmp:wasmJsBrowserDistribution")
    from(clientKmpBuildDir)
    into("src/jvmMain/resources/static/kmp")
}
```

---

## Resumo de Arquivos Novos

| Arquivo                                                         | Fase |
|-----------------------------------------------------------------|------|
| `client-kmp/build.gradle.kts`                                  | 1    |
| `client-kmp/src/commonMain/.../state/AppState.kt`              | 2    |
| `client-kmp/src/commonMain/.../services/WebsocketService.kt`   | 2    |
| `client-kmp/src/commonMain/.../services/PacketHandler.kt`      | 2    |
| `client-kmp/src/commonMain/.../services/SessionManager.kt`     | 2    |
| `client-kmp/src/commonMain/.../services/WebRtcManager.kt`      | 5    |
| `client-kmp/src/commonMain/.../media/MediaCapture.kt` (expect) | 5    |
| `client-kmp/src/commonMain/.../ui/App.kt`                      | 3    |
| `client-kmp/src/commonMain/.../ui/theme/Theme.kt`              | 3    |
| `client-kmp/src/commonMain/.../ui/screens/JoinScreen.kt`       | 3    |
| `client-kmp/src/commonMain/.../ui/screens/RoomScreen.kt`       | 3    |
| `client-kmp/src/commonMain/.../ui/components/ChatPanel.kt`     | 3    |
| `client-kmp/src/commonMain/.../ui/components/ChatBubble.kt`    | 3    |
| `client-kmp/src/commonMain/.../ui/components/UserListPanel.kt` | 3    |
| `client-kmp/src/commonMain/.../ui/components/UserListItem.kt`  | 3    |
| `client-kmp/src/commonMain/.../ui/components/ScreenShareArea.kt` | 3  |
| `client-kmp/src/commonMain/.../ui/components/Toolbar.kt`       | 3    |
| `client-kmp/src/desktopMain/kotlin/Main.kt`                    | 4    |
| `client-kmp/src/wasmJsMain/kotlin/Main.kt`                     | 4    |
| `client-kmp/src/desktopMain/.../media/MediaCapture.kt` (actual) | 5   |
| `client-kmp/src/wasmJsMain/.../media/MediaCapture.kt` (actual) | 5    |

## Arquivos Modificados

| Arquivo                        | Fase | Alteração                       |
|--------------------------------|------|---------------------------------|
| `libs.versions.toml`           | 1    | Compose + novas libs            |
| `settings.gradle.kts`          | 1    | `include(":client-kmp")`        |
| `common/build.gradle.kts`      | 1    | Adicionar target `wasmJs`       |
| `server/build.gradle.kts`      | 6    | Task `copyClientKmpToServer`    |

---

## Ordem de Execução Recomendada

```
Fase 1 → Fase 2 → Fase 3 → Fase 4 → (E2E chat-only) → Fase 0 → Fase 5 → Fase 6
```

A Fase 0 (fork webrtc-kmp) é intencionalmente adiada para após o MVP funcional de chat.
Isso permite validar toda a stack Compose + WebSocket antes de adicionar a complexidade do WebRTC.
