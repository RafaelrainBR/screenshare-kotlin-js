package decorators

import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamTrack
import kotlin.js.Json
import kotlin.js.Promise

external class RTCSessionDescription(
    data: Json,
)

external class RTCIceCandidate(
    candidateInfo: Json,
)

class RTCPeerConnectionDecorator(
    private val windowRTCPeerConnection: dynamic,
) {
    val currentRemoteDescription: Any?
        get() = windowRTCPeerConnection.currentRemoteDescription

    fun createOffer(): Promise<Json> = windowRTCPeerConnection.createOffer() as Promise<Json>

    fun createAnswer(): Promise<Json> = windowRTCPeerConnection.createAnswer() as Promise<Json>

    fun addTrack(
        track: MediaStreamTrack,
        localStream: MediaStream,
    ) {
        windowRTCPeerConnection.addTrack(track, localStream)
    }

    fun getSenders(): Array<dynamic> = windowRTCPeerConnection.getSenders() as Array<dynamic>

    fun hasTrack(track: MediaStreamTrack): Boolean {
        val senders = getSenders()
        return senders.any { sender ->
            sender.track?.id == track.id
        }
    }

    fun onTrack(block: (streams: Array<MediaStream>) -> Unit) {
        windowRTCPeerConnection.addEventListener("track") { event ->
            console.log("received track event ", event)
            try {
                block(event.streams as Array<MediaStream>)
            } catch (e: Throwable) {
                console.error("Error in onTrack handler: ", e)
            }
        }
    }

    fun onIceCandidateAdd(block: (Json?) -> Unit) {
        windowRTCPeerConnection.onicecandidate = { event: dynamic ->
            console.log("new ice event ${JSON.stringify(event)}")
            console.log("new ice candidate ${JSON.stringify(event.candidate)}")
            block(event.candidate.unsafeCast<Json?>())
        }
    }

    fun setLocalDescription(description: Json): Promise<Json> =
        windowRTCPeerConnection.setLocalDescription(description) as Promise<Json>

    fun setRemoteDescription(description: RTCSessionDescription): Promise<Json> =
        windowRTCPeerConnection.setRemoteDescription(description) as Promise<Json>

    fun addIceCandidate(candidate: RTCIceCandidate): Promise<Json> =
        windowRTCPeerConnection.addIceCandidate(candidate) as Promise<Json>

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

        private fun instantiate(): dynamic =
            js(
                """
                new RTCPeerConnection({
                  iceServers: [
                    {
                      urls: ['stun:stun.l.google.com:19302', 'stun:stun1.l.google.com:19302', 'stun:stun2.l.google.com:19302'],
                    },
                    {
                      urls: ['turn:openrelay.metered.ca:80', 'turn:openrelay.metered.ca:443', 'turn:openrelay.metered.ca:443?transport=tcp'],
                      username: 'openrelayproject',
                      credential: 'openrelayproject',
                    },
                  ],
                  iceCandidatePoolSize: 10,
                });
            """,
            )
    }
}

fun createRTCIceCandidate(
    candidate: String,
    sdpMid: String,
    sdpMLineIndex: Int,
): Json =
    js(
        "new RTCIceCandidate({candidate: candidate, sdpMid: sdpMid, sdpMLineIndex: sdpMLineIndex})",
    ).unsafeCast<Json>()
