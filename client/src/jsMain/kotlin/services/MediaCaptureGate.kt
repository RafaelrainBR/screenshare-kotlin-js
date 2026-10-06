package services

/** Invalidates permission requests already in flight without resuming capture on unlock. */
class MediaCaptureGate {
    var available = true
        private set
    private var generation = 0

    fun request(): () -> Boolean {
        val requestedGeneration = generation
        return { available && requestedGeneration == generation }
    }

    fun invalidate() { generation++ }

    fun setAvailable(value: Boolean) {
        available = value
        if (!value) invalidate()
    }
}
