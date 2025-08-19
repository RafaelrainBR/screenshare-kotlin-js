package screenshare.client

import dev.gitlive.firebase.firestore.externals.addDoc
import dev.gitlive.firebase.firestore.externals.onSnapshot
import dev.gitlive.firebase.firestore.js
import kotlinx.browser.document
import kotlinx.browser.window
import kotlinx.coroutines.CoroutineExceptionHandler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.await
import kotlinx.coroutines.launch
import org.w3c.dom.HTMLButtonElement
import org.w3c.dom.HTMLInputElement
import org.w3c.dom.HTMLVideoElement
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamConstraints
import screenshare.client.decorators.RTCPeerConnectionDecorator
import screenshare.client.extensions.getDisplayMedia
import screenshare.client.extensions.getTypedElement
import screenshare.client.external.RTCIceCandidate
import screenshare.client.external.RTCSessionDescription
import screenshare.client.service.FirebaseService
import screenshare.client.service.getFirebaseConfig

fun main() {
    val displayButton = document.getTypedElement<HTMLButtonElement>("start-sharing-button")
    val createCallButton = document.getTypedElement<HTMLButtonElement>("create-call-button")
    val joinCallButton = document.getTypedElement<HTMLButtonElement>("join-call-button")
    val signOutButton = document.getTypedElement<HTMLButtonElement>("sign-out-button")

    val localPlayer = document.getTypedElement<HTMLVideoElement>("local-video")
    val remotePlayer = document.getTypedElement<HTMLVideoElement>("remote-video")

    val generatedCallInput = document.getTypedElement<HTMLInputElement>("generated-call-input")
    val joinCallInput = document.getTypedElement<HTMLInputElement>("join-call-input")

    var localStream: MediaStream?
    var remoteStream: MediaStream?

    val coroutineExceptionHandler =
        CoroutineExceptionHandler { _, throwable ->
            console.log("coroutineExceptionHandler got exception", throwable)
        }
    val coroutineScope = CoroutineScope(Dispatchers.Main + SupervisorJob() + coroutineExceptionHandler)

    val peerConnection = RTCPeerConnectionDecorator.create()
    val firebaseService = FirebaseService.create(getFirebaseConfig())

    displayButton.onclick = {
        coroutineScope.launch {
            startScreenShare { stream ->
                displayButton.disabled = true
                createCallButton.disabled = false
                joinCallButton.disabled = false

                localStream = stream

                localPlayer.srcObject = localStream
                remoteStream = MediaStream()
                remotePlayer.srcObject = remoteStream
                peerConnection.onTrackAdd { track ->
                    remoteStream!!.addTrack(track)
                }

                localStream!!.getTracks().forEach { track -> peerConnection.addTrack(track, localStream!!) }
            }
        }
    }

    createCallButton.onclick = {
        coroutineScope.launch {
            createCallButton.disabled = true

            createCall(
                firebaseService = firebaseService,
                peerConnection = peerConnection,
                onGenerateId = { id -> generatedCallInput.value = id },
            )
            signOutButton.disabled = false
            joinCallButton.disabled = true
        }
    }

    joinCallButton.onclick = {
        coroutineScope.launch {
            val callId = joinCallInput.value
            if (callId.isBlank()) return@launch

            remoteStream = MediaStream()
            remotePlayer.srcObject = remoteStream
            peerConnection.onTrackAdd { track ->
                remoteStream!!.addTrack(track)
            }

            joinCall(
                firebaseService = firebaseService,
                peerConnection = peerConnection,
                callId = callId,
            )

            joinCallButton.disabled = true
            signOutButton.disabled = false
        }
    }
}

private fun startScreenShare(onStart: (MediaStream) -> Unit) {
    window.navigator.mediaDevices
        .getDisplayMedia(mediaStreamConstraints)
        .then { stream ->
            onStart(stream)
        }.catch { error ->
            console.log("Error accessing screen sharing: ", error)
        }
}

private suspend fun createCall(
    firebaseService: FirebaseService,
    peerConnection: RTCPeerConnectionDecorator,
    onGenerateId: (String) -> Unit,
) {
    val callDoc = firebaseService.createCallDoc()
    onGenerateId(callDoc.id)

    val offerCandidates = callDoc.collection("offerCandidates")
    val answerCandidates = callDoc.collection("answerCandidates")

    peerConnection.onIceCandidateAdd { iceCandidate ->
        if (iceCandidate != null) {
            addDoc(offerCandidates.js, iceCandidate.asDynamic().toJSON() as Any)
        }
    }

    val offerDescription = peerConnection.createOffer().await()
    peerConnection.setLocalDescription(offerDescription).await()

    val offerDescriptionAsMap =
        mapOf(
            "type" to offerDescription.type,
            "sdp" to offerDescription.sdp,
        )
    callDoc.set(mapOf("offer" to offerDescriptionAsMap))

    onSnapshot(callDoc.js, next = { snapshot ->
        val data = snapshot.data()
        if (peerConnection.currentRemoteDescription == null && data != null && data.asDynamic().answer != null) {
            val answerDescription = RTCSessionDescription(data.asDynamic().answer)
            peerConnection.setRemoteDescription(answerDescription)
        }
    }, error = { error ->
        console.log("Error fetching document", error)
    })

    onSnapshot(answerCandidates.js, next = { snapshot ->
        snapshot.docChanges().forEach { change ->
            if (change.asDynamic().type == "added") {
                val candidate = RTCIceCandidate(change.doc.data())
                peerConnection.addIceCandidate(candidate)
            }
        }
    }, error = { error ->
        console.log("Error fetching document", error)
    })
}

private suspend fun joinCall(
    firebaseService: FirebaseService,
    peerConnection: RTCPeerConnectionDecorator,
    callId: String,
) {
    val callDoc = firebaseService.getCallDoc(callId)
    val offerCandidates = callDoc.collection("offerCandidates")
    val answerCandidates = callDoc.collection("answerCandidates")

    peerConnection.onIceCandidateAdd { iceCandidate ->
        if (iceCandidate != null) {
            addDoc(answerCandidates.js, iceCandidate.asDynamic().toJSON() as Any)
        }
    }

    val callData = callDoc.get().js.data().asDynamic()

    val offerDescription = callData.offer
    peerConnection.setRemoteDescription(RTCSessionDescription(offerDescription)).await()

    val answerDescription = peerConnection.createAnswer().await()
    peerConnection.setLocalDescription(answerDescription).await()

    val answerDescriptionAsMap =
        mapOf(
            "type" to answerDescription.type,
            "sdp" to answerDescription.sdp,
        )

    callDoc.set(mapOf("answer" to answerDescriptionAsMap), merge = true)

    onSnapshot(offerCandidates.js, next = { snapshot ->
        snapshot.docChanges().forEach { change ->
            if (change.asDynamic().type == "added") {
                val candidate = RTCIceCandidate(change.doc.data())
                peerConnection.addIceCandidate(candidate)
            }
        }
    }, error = { error ->
        console.log("Error fetching document", error)
    })
}

val mediaStreamConstraints =
    MediaStreamConstraints(
        video =
            mapOf(
                "width" to mapOf("ideal" to 1920),
                "height" to mapOf("ideal" to 1080),
                "frameRate" to 30,
                "resizeMode" to "crop-and-scale",
            ),
        audio =
            mapOf(
                "sampleSize" to 32,
                "sampleRate" to 48000,
                "echoCancellation" to false,
                "volume" to 1.0,
            ),
    )
