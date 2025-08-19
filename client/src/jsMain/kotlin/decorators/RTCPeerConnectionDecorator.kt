package decorators

import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamTrack
import kotlin.js.Json
import kotlin.js.Promise

external class RTCSessionDescription(data: Json)

external class RTCIceCandidate(candidateInfo: Json)

class RTCPeerConnectionDecorator(private val windowRTCPeerConnection: dynamic) {
    val currentRemoteDescription: Any?
        get() = windowRTCPeerConnection.currentRemoteDescription

    fun createOffer(): Promise<Json> {
        return windowRTCPeerConnection.createOffer() as Promise<Json>
    }

    fun createAnswer(): Promise<Json> {
        return windowRTCPeerConnection.createAnswer() as Promise<Json>
    }

    fun addTrack(
        track: MediaStreamTrack,
        localStream: MediaStream,
    ) {
        windowRTCPeerConnection.addTrack(track, localStream)
    }

    fun onTrack(block: (streams: List<MediaStream>) -> Unit) {
        windowRTCPeerConnection.addEventListener("track") { event ->
            console.log("received track event ", event)
            block(event.streams as List<MediaStream>)
        }
    }

    fun onIceCandidateAdd(block: (Json?) -> Unit) {
        windowRTCPeerConnection.onicecandidate = { event: dynamic ->
            console.log("new ice event ${JSON.stringify(event)}")
            console.log("new ice candidate ${JSON.stringify(event.candidate)}")
            block(event.candidate as Json?)
        }
    }

    fun setLocalDescription(description: Json): Promise<Json> {
        return windowRTCPeerConnection.setLocalDescription(description) as Promise<Json>
    }

    fun setRemoteDescription(description: RTCSessionDescription): Promise<Json> {
        return windowRTCPeerConnection.setRemoteDescription(description) as Promise<Json>
    }

    fun addIceCandidate(candidate: RTCIceCandidate): Promise<Json> {
        return windowRTCPeerConnection.addIceCandidate(candidate) as Promise<Json>
    }

    fun close() {
        windowRTCPeerConnection.close()
    }

    companion object {
        fun create(): RTCPeerConnectionDecorator {
            val peerConnection = instantiate()
            peerConnection.oniceconnectionstatechange = {
                console.log("ICE connection state: ${peerConnection.iceConnectionState}")
            }
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

fun createRTCIceCandidate(
    candidate: String,
    sdpMid: String,
    sdpMLineIndex: Int
): Json = js("new RTCIceCandidate({candidate: candidate, sdpMid: sdpMid, sdpMLineIndex: sdpMLineIndex})") as Json