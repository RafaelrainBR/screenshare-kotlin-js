import kotlinx.browser.document
import org.w3c.dom.HTMLButtonElement
import org.w3c.dom.HTMLDivElement
import org.w3c.dom.HTMLInputElement
import org.w3c.dom.HTMLSpanElement
import org.w3c.dom.HTMLUListElement
import org.w3c.dom.HTMLVideoElement

object Elements {
    val joinScreen = document.getElementById("join-screen") as HTMLDivElement
    val appScreen = document.getElementById("app-screen") as HTMLDivElement
    val usernameInput = document.getElementById("username") as HTMLInputElement
    val roomIdInput = document.getElementById("roomId") as HTMLInputElement
    val joinButton = document.getElementById("joinBtn") as HTMLButtonElement
    val chatMessages = document.getElementById("chat-messages") as HTMLDivElement
    val currentRoomId = document.getElementById("current-room-id") as HTMLSpanElement
    val screenVideo = document.getElementById("screen-video") as HTMLVideoElement
    val videoContainer = document.getElementById("video-container") as HTMLDivElement
    val noScreenMessage = document.getElementById("no-screen-message") as HTMLDivElement
    val userList = document.getElementById("user-list") as HTMLUListElement
    val userListButton = document.getElementById("userListToggle") as HTMLButtonElement
    val userListDropdown = document.getElementById("userListDropdown") as HTMLDivElement
    val sendMessageButton = document.getElementById("send-message-btn") as HTMLButtonElement
    val messageInput = document.getElementById("message-input") as HTMLInputElement
    val micToggle = document.getElementById("micToggle") as HTMLButtonElement
}
