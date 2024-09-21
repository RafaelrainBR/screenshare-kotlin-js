package screenshare.client.decorators

import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamTrack
import screenshare.client.external.RTCSessionDescription
import kotlin.js.Promise

class RTCPeerConnectionDecorator(private val windowRTCPeerConnection: dynamic) {
    val currentRemoteDescription: Any?
        get() = windowRTCPeerConnection.currentRemoteDescription

    fun createOffer(): Promise<dynamic> {
        return windowRTCPeerConnection.createOffer() as Promise<dynamic>
    }

    fun createAnswer(): Promise<dynamic> {
        return windowRTCPeerConnection.createAnswer() as Promise<dynamic>
    }

    fun addTrack(
        track: MediaStreamTrack,
        localStream: MediaStream,
    ) {
        windowRTCPeerConnection.addTrack(track, localStream)
    }

    fun onTrackAdd(block: (track: MediaStreamTrack) -> Unit) {
        windowRTCPeerConnection.addEventListener("track") { event ->
            console.log("received track event ", event)
            event.streams[0].getTracks().forEach { track ->
                block(track as MediaStreamTrack)
            }
        }
    }

    fun onIceCandidateAdd(block: (Any?) -> Unit) {
        windowRTCPeerConnection.addEventListener("icecandidate") { event ->
            block(event.candidate)
        }
    }

    fun setLocalDescription(description: dynamic): Promise<dynamic> {
        return windowRTCPeerConnection.setLocalDescription(description) as Promise<dynamic>
    }

    fun setRemoteDescription(description: RTCSessionDescription): Promise<dynamic> {
        return windowRTCPeerConnection.setRemoteDescription(description) as Promise<dynamic>
    }

    fun addIceCandidate(candidate: dynamic) {
        windowRTCPeerConnection.addIceCandidate(candidate)
    }

    fun close() {
        windowRTCPeerConnection.close()
    }

    companion object {
        fun create(): RTCPeerConnectionDecorator {
            val peerConnection = instantiate()
            return RTCPeerConnectionDecorator(peerConnection)
        }

        private fun instantiate(): dynamic {
            return js(
                """
                new RTCPeerConnection({
                  iceServers: [
                    {
                      urls: ['stun:stun1.l.google.com:19302', 'stun:stun2.l.google.com:19302'],
                    },
                  ],
                  iceCandidatePoolSize: 10,
                });
            """,
            )
        }
    }
}
