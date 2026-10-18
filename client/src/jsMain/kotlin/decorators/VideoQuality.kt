package decorators

/** Seeds bandwidth estimation without imposing a minimum bitrate on slow connections. */
fun improveVideoStartupSdp(sdp: String): String =
    sdp.split(Regex("(?=^m=)", RegexOption.MULTILINE)).joinToString("") { section ->
        if (!section.startsWith("m=video ")) return@joinToString section
        var result = section
        val newline = if ("\r\n" in section) "\r\n" else "\n"
        val codecs =
            Regex("^a=rtpmap:(\\d+) (VP8|VP9|H264|AV1)/90000", setOf(RegexOption.MULTILINE, RegexOption.IGNORE_CASE))
        codecs.findAll(section).forEach { codec ->
            val payload = codec.groupValues[1]
            val fmtp = Regex("^a=fmtp:$payload ([^\\r\\n]*)", RegexOption.MULTILINE)
            val existing = fmtp.find(result)
            // shortcut: Chromium understands this hint; replace it when WebRTC exposes a standard startup bitrate API.
            val hint = "x-google-start-bitrate=1500"
            if (existing == null) {
                result = result.trimEnd('\r', '\n') + newline + "a=fmtp:$payload $hint" + newline
            } else if (!existing.value.contains("x-google-start-bitrate=", ignoreCase = true)) {
                result = fmtp.replace(result) { "${it.value};$hint" }
            }
        }
        result
    }
