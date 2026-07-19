package screenshare.clientkmp.services

data class AudioDevice(
    val id: String,
    val label: String,
)

data class DeviceSettings(
    val micDeviceId: String? = null,
    val outputDeviceId: String? = null,
    val micVolume: Float = 1f,
    val outputVolume: Float = 1f,
)
