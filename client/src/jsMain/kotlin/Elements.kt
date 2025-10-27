import kotlinx.browser.document
import org.w3c.dom.HTMLButtonElement
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLInputElement
import org.w3c.dom.HTMLSpanElement
import org.w3c.dom.HTMLVideoElement

object Elements {
    val joinScreen = document.getElementById("join-screen") as HTMLElement
    val appScreen = document.getElementById("app-screen") as HTMLElement
    val usernameInput = document.getElementById("username") as HTMLInputElement
    val roomIdInput = document.getElementById("roomId") as HTMLInputElement
    val joinButton = document.getElementById("joinBtn") as HTMLButtonElement
    val chatMessages = document.getElementById("chat-messages") as HTMLElement
    val currentRoomId = document.getElementById("current-room-id") as HTMLElement
    val screenVideo = document.getElementById("screen-video") as HTMLVideoElement
    val videoContainer = document.getElementById("video-container") as HTMLElement
    val noScreenMessage = document.getElementById("no-screen-message") as HTMLElement
    val userList = document.getElementById("user-list") as HTMLElement
    val userCount = document.getElementById("participant-count") as HTMLSpanElement
    val sendMessageButton = document.getElementById("send-message-btn") as HTMLButtonElement
    val messageInput = document.getElementById("message-input") as HTMLInputElement
    val micToggle = document.getElementById("micToggle") as HTMLButtonElement
}
