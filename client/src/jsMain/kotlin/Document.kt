import kotlinx.browser.document
import kotlinx.browser.window
import org.w3c.dom.HTMLDivElement
import org.w3c.dom.HTMLLIElement
import org.w3c.dom.HTMLParagraphElement
import org.w3c.dom.url.URL
import screenshare.common.ChatMessage
import screenshare.common.SocketUser
import kotlin.js.Date

private fun String.getUsernameInitials(): String {
    return this.split(" ")
        .mapNotNull { it.firstOrNull()?.toString()?.uppercase() }
        .take(2).joinToString("")
}

fun updateUserList(users: List<SocketUser>) {
    userList = users
    Elements.userList.innerHTML = ""
    users.forEach { user ->
        val li = document.createElement("li") as HTMLLIElement

        val initials = user.username.getUsernameInitials()

        val isCurrentUser = user.username == localUsername

        // TODO: Sanitize username to prevent XSS
        li.innerHTML =
            """
            <div class="flex items-center gap-3 p-2 rounded-lg hover:bg-secondary/50 transition-all cursor-pointer group">
                <div class="w-8 h-8 rounded-full ${if (isCurrentUser) "bg-primary/20" else "bg-secondary"} flex items-center justify-center ring-2 ring-transparent group-hover:ring-primary/30 transition-all">
                    <span class="text-xs font-semibold ${if (isCurrentUser) "text-primary" else "text-muted-foreground"}">$initials</span>
                </div>
                <div class="flex-1 min-w-0">
                    <div class="text-sm font-medium truncate">${user.username}${if (isCurrentUser) " (Você)" else ""}</div>
                </div>
                <div class="speaking-indicator"></div>
            </div>
            """.trimIndent()

        Elements.userList.appendChild(li)
    }
    Elements.userCount.textContent = users.size.toString()
}

fun addMessageToChat(message: ChatMessage) {
    val messageElement = document.createElement("div") as HTMLDivElement

    val formattedDate =
        Date(message.timestamp)
            .toLocaleTimeString("pt-BR", js("({ hour: '2-digit', minute: '2-digit' })"))

    val isCurrentUser = message.username == localUsername

    val initials = message.username.getUsernameInitials()

    messageElement.className = "annimate-fade-in"

    val wrapper = document.createElement("div") as HTMLDivElement
    wrapper.className = "flex gap-3 ${if (isCurrentUser) "justify-end" else ""}"

    if (isCurrentUser) {
        wrapper.innerHTML =
            """
            <div class="flex flex-col items-end max-w-[85%]">
                <div class="flex items-center gap-2 mb-1">
                    <span class="text-xs text-muted-foreground">$formattedDate</span>
                    <span class="text-xs font-semibold text-primary">Você</span>
                </div>
                <div class="px-4 py-2 bg-primary text-primary-foreground rounded-2xl rounded-tr-sm shadow-sm">
                    <p class="text-sm"></p>
                </div>
            </div>
            <div class="w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0 ring-2 ring-primary/30">
                <span class="text-xs font-semibold text-primary">$initials</span>
            </div>
            """.trimIndent()
    } else {
        // TODO: Sanitize message username to prevent XSS
        wrapper.innerHTML =
            """
            <div class="w-8 h-8 rounded-full bg-secondary flex items-center justify-center flex-shrink-0">
                <span class="text-xs font-semibold text-muted-foreground">$initials</span>
            </div>
            <div class="flex flex-col max-w-[85%]">
                <div class="flex items-center gap-2 mb-1">
                    <span class="text-xs font-semibold">${message.username}</span>
                    <span class="text-xs text-muted-foreground">$formattedDate</span>
                </div>
                <div class="px-4 py-2 bg-secondary/50 text-foreground rounded-2xl rounded-tl-sm shadow-sm">
                    <p class="text-sm"></p>
                </div>
            </div>
            """.trimIndent()
    }

    val paragraph = wrapper.querySelector("p") as HTMLParagraphElement
    paragraph.textContent = message.content

    messageElement.appendChild(wrapper)
    Elements.chatMessages.appendChild(messageElement)

    messageElement.scrollIntoView(js("{ behavior: 'smooth', block: 'end' }"))
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
