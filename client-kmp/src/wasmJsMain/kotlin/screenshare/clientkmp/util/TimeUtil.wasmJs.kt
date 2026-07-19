package screenshare.clientkmp.util

@OptIn(kotlin.js.ExperimentalWasmJsInterop::class)
@JsFun("() => Date.now()")
external fun jsDateNow(): Double

actual fun currentTimeMillis(): Long = jsDateNow().toLong()
