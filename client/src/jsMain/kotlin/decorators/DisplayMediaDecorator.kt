package decorators

import org.w3c.dom.mediacapture.MediaDevices
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamConstraints
import kotlin.js.Promise

fun MediaDevices.getDisplayMedia(): Promise<MediaStream> = this.asDynamic().getDisplayMedia() as Promise<MediaStream>

fun MediaDevices.getDisplayMedia(constraints: MediaStreamConstraints): Promise<MediaStream> =
    this.asDynamic().getDisplayMedia(constraints) as Promise<MediaStream>
