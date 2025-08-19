package screenshare.client.extensions

import org.w3c.dom.Document
import org.w3c.dom.Element

inline fun <reified T : Element> Document.getTypedElement(elementId: String): T {
    val element = requireNotNull(getElementById(elementId)) { "$elementId not found" }
    require(element is T) { "$elementId is not of type ${T::class.simpleName}" }

    return element
}
