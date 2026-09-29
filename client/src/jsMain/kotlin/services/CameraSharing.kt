package services

import kotlin.js.json
import kotlinx.browser.window
import kotlinx.coroutines.await
import org.w3c.dom.mediacapture.MediaStream
import org.w3c.dom.mediacapture.MediaStreamConstraints
import ui.InterfaceMutations

class CameraSharing(
    private val resolveUsername: (String) -> String,
    private val localUsername: String,
) {
    var localCameraStream: MediaStream? = null
    val remoteCameraStreams: MutableMap<String, MediaStream> = mutableMapOf()

    suspend fun setupLocalCameraStream(
        recreatePeerConnections: () -> Unit,
        onStreamEnd: () -> Unit,
        deviceId: String? = null,
    ) {
        val previousStream = localCameraStream
        val nextStream =
            window.navigator.mediaDevices
                .getUserMedia(buildMediaStreamConstraints(deviceId))
                .await()
        previousStream?.getVideoTracks()?.forEach { it.onended = null }
        previousStream?.getTracks()?.forEach { it.stop() }
        localCameraStream = nextStream
        val videoTrack = localCameraStream?.getVideoTracks()?.firstOrNull()
        if (videoTrack != null) {
            videoTrack.onended = {
                localCameraStream = null
                recreatePeerConnections()
                InterfaceMutations.removeCameraTile(CAMERA_LOCAL_TILE_ID)
                InterfaceMutations.updateCameraControls(isLocalCameraOn = false)
                onStreamEnd()
            }

            recreatePeerConnections()
        }
        InterfaceMutations.addOrUpdateCameraTile(
            tileId = CAMERA_LOCAL_TILE_ID,
            stream = localCameraStream!!,
            username = localUsername,
            isLocal = true,
        )
        InterfaceMutations.updateCameraControls(isLocalCameraOn = true)
    }

    fun handleRemoteCamera(
        socketId: String,
        stream: MediaStream,
    ) {
        val previous = remoteCameraStreams[socketId]
        if (previous != null && previous.id != stream.id) {
            previous.getTracks().forEach { it.stop() }
        }
        remoteCameraStreams[socketId] = stream
        InterfaceMutations.addOrUpdateCameraTile(
            tileId = cameraTileId(socketId),
            stream = stream,
            username = resolveUsername(socketId),
            isLocal = false,
        )
    }

    fun stopRemoteCamera(socketId: String) {
        remoteCameraStreams.remove(socketId)?.getTracks()?.forEach { it.stop() }
        InterfaceMutations.removeCameraTile(cameraTileId(socketId))
    }

    fun stopCameraSharing(recreatePeerConnections: () -> Unit) {
        localCameraStream?.getTracks()?.forEach { track -> track.stop() }
        localCameraStream = null
        recreatePeerConnections()
        InterfaceMutations.removeCameraTile(CAMERA_LOCAL_TILE_ID)
        InterfaceMutations.updateCameraControls(isLocalCameraOn = false)
    }

    private fun buildMediaStreamConstraints(deviceId: String?): MediaStreamConstraints {
        // IMPORTANTE: kotlin.js.json (objeto JS plano), não um Map do Kotlin —
        // o browser lê width/height/frameRate como propriedades próprias do objeto.
        val video: dynamic =
            json(
                "width" to json("ideal" to 1280),
                "height" to json("ideal" to 720),
                "frameRate" to json("ideal" to 30, "max" to 30),
                "resizeMode" to "crop-and-scale",
            )
        if (!deviceId.isNullOrBlank()) video["deviceId"] = json("exact" to deviceId)
        return MediaStreamConstraints(video = video)
    }

    companion object {
        const val CAMERA_LOCAL_TILE_ID = "camera-local"

        fun cameraTileId(socketId: String): String = "camera-$socketId"
    }
}
