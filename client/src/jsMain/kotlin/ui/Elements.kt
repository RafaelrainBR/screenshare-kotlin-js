package ui

import kotlinx.browser.document
import org.w3c.dom.HTMLButtonElement
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLInputElement
import org.w3c.dom.HTMLSelectElement

object Elements {
    val joinScreen = getElement<HTMLElement>("join-screen")
    val appScreen = getElement<HTMLElement>("app-screen")
    val usernameInput = getElement<HTMLInputElement>("username")
    val roomIdInput = getElement<HTMLInputElement>("roomId")
    val joinButton = getElement<HTMLButtonElement>("joinBtn")
    val chatMessages = getElement<HTMLElement>("chat-messages")
    val currentRoomId = getElement<HTMLElement>("current-room-id")
    val noScreenMessage = getElement<HTMLElement>("no-screen-message")
    val screensStage = getElement<HTMLElement>("screens-stage")
    val screensMain = getElement<HTMLElement>("screens-main")
    val screensRail = getElement<HTMLElement>("screens-rail")
    val userList = getElement<HTMLElement>("user-list")
    val userCount = getElement<HTMLElement>("participant-count")
    val sendMessageButton = getElement<HTMLButtonElement>("send-message-btn")
    val messageInput = getElement<HTMLInputElement>("message-input")
    val micToggle = getElement<HTMLButtonElement>("micToggle")
    val shareScreenButton = getElement<HTMLButtonElement>("shareScreenBtn")
    val stopScreenShareButton = getElement<HTMLButtonElement>("stopSharingBtn")
    val changeDesktopAudioButton = getElement<HTMLButtonElement>("changeDesktopAudioBtn")
    val cameraButton = getElement<HTMLButtonElement>("cameraBtn")
    val stopCameraButton = getElement<HTMLButtonElement>("stopCameraBtn")
    val inputDevices = getElement<HTMLSelectElement>("inputDevices")
    val outputDevices = getElement<HTMLSelectElement>("outputDevices")
    val qualityModal = getElement<HTMLElement>("quality-modal")
    val confirmShare = getElement<HTMLButtonElement>("confirm-share")
    val cancelShare = getElement<HTMLButtonElement>("cancel-share")
    val desktopWizardStepOne = getElement<HTMLElement>("desktop-wizard-step-one")
    val desktopWizardStepTwo = getElement<HTMLElement>("desktop-wizard-step-two")
    val desktopAudioSourceGrid = getElement<HTMLElement>("desktop-audio-source-grid")
    val desktopAudioPickerState = getElement<HTMLElement>("desktop-audio-picker-state")
    val desktopAudioRefresh = getElement<HTMLButtonElement>("desktop-audio-refresh")
    val desktopAudioThumbnail = getElement<HTMLElement>("desktop-audio-thumbnail")
    val desktopBackToAudioModes = getElement<HTMLButtonElement>("desktop-back-to-audio-modes")
}

private inline fun <reified T : HTMLElement> getElement(id: String): T =
    runCatching {
        document.getElementById(id) as T
    }.getOrElse {
        throw IllegalStateException("Element with id '$id' not found or is not of type ${T::class.simpleName}")
    }
