package ui

import getUsernameInitials
import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.await
import org.w3c.dom.HTMLAudioElement
import org.w3c.dom.HTMLDivElement
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLOptionElement
import org.w3c.dom.HTMLParagraphElement
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.url.URL
import screenshare.common.ChatMessage
import ui.mutations.UserListMutations
import kotlin.js.Date

object InterfaceMutations {
    var selectedOutputDeviceId: String? = null
    fun navigateToRoomScreen(
        roomId: String,
        username: String,
    ) {
        Elements.joinScreen.classList.add("hidden")
        Elements.appScreen.classList.remove("hidden")

        val actualUrl = URL(window.location.href)
        val newUrl = "${actualUrl.origin}/?roomId=$roomId"
        window.history.pushState(null, "", newUrl)
    }

    fun addMessageToChat(
        message: ChatMessage,
        localUsername: String,
    ) {
        val messageElement = document.createElement("div") as HTMLDivElement

        val formattedDate =
            Date(message.timestamp)
                .toLocaleTimeString("pt-BR", js("({ hour: '2-digit', minute: '2-digit' })"))

        val isCurrentUser = message.username == localUsername

        val initials = message.username.getUsernameInitials()

        messageElement.className = "animate-fade-in"

        val wrapper = document.createElement("div") as HTMLDivElement
        wrapper.className = "chat ${if (isCurrentUser) "chat-end" else "chat-start"}"

        val avatarDiv = document.createElement("div") as HTMLDivElement
        avatarDiv.className = "chat-image avatar placeholder"
        avatarDiv.innerHTML =
            """
            <div class="w-10 rounded-full ${if (isCurrentUser) "bg-primary text-primary-content" else "bg-base-300 text-base-content"}">
                <span class="text-xs font-semibold">$initials</span>
            </div>
            """.trimIndent()
        wrapper.appendChild(avatarDiv)

        val headerDiv = document.createElement("div") as HTMLDivElement
        headerDiv.className = "chat-header"
        headerDiv.innerHTML =
            """
            ${if (isCurrentUser) "Você" else message.username}
            <time class="text-xs opacity-50 ml-2">$formattedDate</time>
            """.trimIndent()
        wrapper.appendChild(headerDiv)

        val bubbleDiv = document.createElement("div") as HTMLDivElement
        bubbleDiv.className = "chat-bubble ${if (isCurrentUser) "chat-bubble-primary" else "bg-base-300"}"

        val paragraph = document.createElement("p") as HTMLParagraphElement
        paragraph.textContent = message.content
        bubbleDiv.appendChild(paragraph)

        wrapper.appendChild(bubbleDiv)

        messageElement.appendChild(wrapper)
        Elements.chatMessages.appendChild(messageElement)

        messageElement.scrollIntoView(js("{ behavior: 'smooth', block: 'end' }"))
    }

    fun endScreenSharing() {
        Elements.screenVideo.srcObject = null

        Elements.videoContainer.classList.add("hidden")
        Elements.stopScreenShareButton.classList.add("hidden")

        Elements.noScreenMessage.classList.remove("hidden")
        Elements.shareScreenButton.classList.remove("hidden")
    }

    fun addAudioElementForUser(
        userId: String,
        stream: MediaStream,
    ) {
        val audioElement =
            (document.createElement("audio") as HTMLAudioElement).apply {
                id = "remote-audio-$userId"
                srcObject = stream
                autoplay = true
                volume = UserListMutations.userVolumes[userId]?.div(100.0) ?: 1.0
            }
        document.body?.appendChild(audioElement)

        if (!selectedOutputDeviceId.isNullOrBlank()) {
            runCatching { audioElement.asDynamic().setSinkId(selectedOutputDeviceId) }
        }
    }

    suspend fun populateAudioDevices() {
        val previousInput = Elements.inputDevices.value
        val previousOutput = Elements.outputDevices.value

        val devices = window.navigator.mediaDevices.enumerateDevices().await()
        val inputSelect = Elements.inputDevices
        val outputSelect = Elements.outputDevices

        inputSelect.innerHTML = ""
        outputSelect.innerHTML = ""

        devices.forEach { device ->
            when (device.asDynamic().kind as String) {
                "audioinput" -> {
                    inputSelect.appendChild(
                        createDeviceOption(device.deviceId, device.label.ifBlank { "Microfone" }),
                    )
                }

                "audiooutput" -> {
                    outputSelect.appendChild(
                        createDeviceOption(device.deviceId, device.label.ifBlank { "Alto-falante" }),
                    )
                }

                else -> {}
            }
        }

        if (devices.any { it.deviceId == previousInput }) Elements.inputDevices.value = previousInput
        if (devices.any { it.deviceId == previousOutput }) Elements.outputDevices.value = previousOutput
    }

    fun setOutputDevice(deviceId: String) {
        selectedOutputDeviceId = deviceId
        applyOutputDeviceToElements(deviceId)
    }

    private fun applyOutputDeviceToElements(deviceId: String) {
        val audioElements = document.getElementsByTagName("audio")
        for (i in 0 until audioElements.length) {
            val element = audioElements.item(i) ?: continue
            if (element.id.startsWith("remote-audio-")) {
                runCatching { element.asDynamic().setSinkId(deviceId) }
            }
        }
        runCatching { Elements.screenVideo.asDynamic().setSinkId(deviceId) }
    }

    private fun createDeviceOption(
        deviceId: String,
        label: String,
    ): HTMLOptionElement =
        (document.createElement("option") as HTMLOptionElement).apply {
            value = deviceId
            textContent = label
        }

    fun updateScreenContainer(
        screenStream: MediaStream,
        isInitiator: Boolean,
    ) {
        with(Elements.screenVideo) {
            if (srcObject == null || srcObject.asDynamic().id != screenStream.id) {
                srcObject = screenStream
                muted = isInitiator

                Elements.videoContainer.classList.remove("hidden")
                Elements.noScreenMessage.classList.add("hidden")

                if (isInitiator) {
                    Elements.stopScreenShareButton.classList.remove("hidden")
                    Elements.shareScreenButton.classList.add("hidden")
                } else {
                    Elements.stopScreenShareButton.classList.add("hidden")
                    Elements.shareScreenButton.classList.add("hidden")
                }
            }
        }
    }

    fun updateAudioControls(isMicMuted: Boolean) {
        val micSlash = document.getElementById("micSlash")!!
        val micStatus = document.getElementById("micStatus")!!

        if (isMicMuted) {
            micSlash.classList.remove("hidden")
            micStatus.classList.remove("badge-success")
            micStatus.classList.add("badge-error")
        } else {
            micSlash.classList.add("hidden")
            micStatus.classList.remove("badge-error")
            micStatus.classList.add("badge-success")
        }
    }
}
