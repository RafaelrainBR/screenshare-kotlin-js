import kotlinx.browser.document
import kotlinx.browser.window
import org.w3c.dom.HTMLDivElement
import org.w3c.dom.HTMLLIElement
import org.w3c.dom.url.URL
import screenshare.common.ChatMessage
import screenshare.common.SocketUser
import kotlin.js.Date

fun updateUserList(users: List<SocketUser>) {
    userList = users
    Elements.userList.innerHTML = ""
    users.forEach { user ->
        val li = document.createElement("li") as HTMLLIElement
        li.className = "py-2 px-1 relative"

        li.innerHTML =
            if (user.username == localUsername) {
                """
                <div class="flex items-center">
                    <span class="w-2 h-2 bg-green-500 rounded-full mr-2"></span>
                    <span class="font-bold">${user.username} (Você)</span>
                </div>
                """.trimIndent()
            } else {
                """
                <div class="flex items-center">
                    <span class="w-2 h-2 bg-blue-500 rounded-full mr-2"></span>
                    <span>${user.username}</span>
                </div>
                """.trimIndent()
            }

        Elements.userList.appendChild(li)
    }
}

fun addMessageToChat(message: ChatMessage) {
    val messageElement = document.createElement("div") as HTMLDivElement
    messageElement.classList.add("chat-message")

    val formattedDate = Date(message.timestamp).toLocaleTimeString()

    generateRandomRoomId()

    if (message.username == localUsername) {
        messageElement.classList.add("bg-blue-100")
        messageElement.innerHTML =
            """
            <div class="flex justify-between items-center mb-1">
                <span class="font-bold text-blue-700">Você</span>
                <span class="text-xs text-gray-500">$formattedDate</span>
            </div>
            <p>${message.content}</p>
            """.trimIndent()
    } else {
        messageElement.classList.add("bg-gray-100")
        messageElement.innerHTML =
            """
            <div class="flex justify-between items-center mb-1">
                <span class="font-bold text-gray-700">${message.username}</span>
                <span class="text-xs text-gray-500">$formattedDate</span>
            </div>
            <p>${message.content}</p>
            """.trimIndent()
    }

    Elements.chatMessages.appendChild(messageElement)
    Elements.chatMessages.scrollTop = Elements.chatMessages.scrollHeight.toDouble()
}

suspend fun joinRoom(
    webhookService: WebsocketService,
    roomId: String,
    username: String,
) {
    localUsername = username
    localRoomId = roomId

    Elements.joinScreen.classList.add("hidden")
    Elements.appScreen.classList.remove("hidden")

    Elements.currentRoomId.textContent = localRoomId

    webhookService.joinRoom(roomId = roomId, username = username)

    addMessageToChat(
        ChatMessage(
            username = "Sistema",
            content = "Você entrou na sala $roomId",
            timestamp = Date().getTime().toLong(),
        ),
    )

    val actualUrl = URL(window.location.href)
    val newUrl = "${actualUrl.origin}/?roomId=$roomId"
    window.history.pushState(null, "", newUrl)

    window.navigator.clipboard.writeText(newUrl)
        .then {
            addMessageToChat(
                ChatMessage(
                    username = "Sistema",
                    content = "ID da sala copiado para a área de transferência. Compartilhe com seus amigos! $newUrl",
                    timestamp = Date().getTime().toLong(),
                ),
            )
        }.catch {
            console.error("Erro ao copiar o ID da sala: ", it)
        }
}


