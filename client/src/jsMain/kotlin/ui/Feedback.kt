package ui

import kotlinx.browser.document
import org.w3c.dom.HTMLElement
import org.w3c.dom.HTMLTextAreaElement
import platform.Telemetry
import platform.sendFeedback

/** "Algo deu errado?" in the conversation header: a few words to Sentry, only where it reports. */
object Feedback {
    fun setup() {
        val button = document.getElementById("feedback-btn") as? HTMLElement ?: return
        val dialog = document.getElementById("feedback-dialog") as? HTMLElement ?: return
        val text = document.getElementById("feedback-text") as? HTMLTextAreaElement ?: return
        button.classList.toggle("hidden", !Telemetry.enabled)
        button.addEventListener("click", {
            dialog.classList.remove("hidden")
            text.focus()
        })
        document.getElementById("close-feedback")?.addEventListener("click", { dialog.classList.add("hidden") })
        document.getElementById("send-feedback")?.addEventListener("click", {
            val message = text.value.trim()
            if (message.isEmpty()) {
                text.focus()
                return@addEventListener
            }
            sendFeedback(message)
            text.value = ""
            dialog.classList.add("hidden")
            InterfaceMutations.showError("Obrigado! Recebemos seu relato.")
        })
    }
}
