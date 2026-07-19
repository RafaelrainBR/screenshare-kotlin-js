package screenshare.clientkmp.state

import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.collections.shouldContain
import io.kotest.matchers.collections.shouldNotContain
import io.kotest.matchers.nulls.shouldBeNull
import io.kotest.matchers.nulls.shouldNotBeNull
import io.kotest.matchers.shouldBe

class AppStateSpec : FunSpec({

    test("tela inicial é Join") {
        val appState = AppState()
        appState.screen.value shouldBe Screen.Join
    }

    test("navigateToRoom transiciona para Room") {
        val appState = AppState()
        appState.navigateToRoom("sala1", "Alice")
        val screen = appState.screen.value
        screen shouldBe Screen.Room(RoomState(roomId = "sala1", username = "Alice"))
    }

    test("currentRoom retorna null quando estiver na tela Join") {
        AppState().currentRoom.shouldBeNull()
    }

    test("currentRoom retorna o estado atual da sala") {
        val appState = AppState()
        appState.navigateToRoom("sala1", "Alice")
        appState.currentRoom.shouldNotBeNull()
        appState.currentRoom?.roomId shouldBe "sala1"
        appState.currentRoom?.username shouldBe "Alice"
    }

    test("updateRoom modifica o estado da sala") {
        val appState = AppState()
        appState.navigateToRoom("sala1", "Alice")
        appState.updateRoom { it.copy(isMicMuted = false) }
        appState.currentRoom?.isMicMuted shouldBe false
    }

    test("updateRoom não faz nada na tela Join") {
        val appState = AppState()
        appState.updateRoom { it.copy(isMicMuted = false) }
        appState.screen.value shouldBe Screen.Join
    }

    test("setSpeaking adiciona socketId ao conjunto speakingUsers") {
        val appState = AppState()
        appState.navigateToRoom("sala1", "Alice")
        appState.setSpeaking("socket1", true)
        appState.currentRoom?.speakingUsers!! shouldContain "socket1"
    }

    test("setSpeaking remove socketId quando isSpeaking é false") {
        val appState = AppState()
        appState.navigateToRoom("sala1", "Alice")
        appState.setSpeaking("socket1", true)
        appState.setSpeaking("socket1", false)
        appState.currentRoom?.speakingUsers!! shouldNotContain "socket1"
    }

    test("initialRoomId é armazenado na instância") {
        AppState(initialRoomId = "sala-deep-link").initialRoomId shouldBe "sala-deep-link"
    }

    test("initialRoomId é null por padrão") {
        AppState().initialRoomId.shouldBeNull()
    }
})
