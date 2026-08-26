package decorators

// Upgrades the Opus codec parameters in an SDP so that screen-share audio
// (movies / music) is transmitted at the best possible quality and without
// audible gaps.
//
// Why these parameters fix the "gap / frequencies being cut" symptom:
//  - By default Chrome negotiates Opus in 32kHz mono at a low bitrate and may
//    apply aggressive audio processing that dulls the spectrum.
//  - stereo=1 + sprop-stereo=1: true stereo capture/render (essential for movies).
//  - maxplaybackrate=48000: keep the full sample rate so nothing is downsampled.
//  - ptime=20 / minptime=10: balanced packetization (low latency, no tiny gaps).
//  - useinbandfec=1 + usedtx=1: Opus in-band FEC reconstructs packets lost on
//    the network (very common over TURN/CGNAT), filling the "gaps" in sound.
//    usedtx is required for FEC to work and only applies during silence, so it
//    never degrades continuous audio and saves bandwidth on pauses.
//  - maxaveragebitrate=192000: transparent high-fidelity ceiling for stereo
//    music; the encoder uses less when there is little to encode.
fun upgradeAudioQualitySdp(sdp: String): String {
    val opusPayloadTypes = mutableListOf<String>()
    val rtpmapRegex = Regex("a=rtpmap:(\\d+) opus/48000", RegexOption.IGNORE_CASE)
    rtpmapRegex.findAll(sdp).forEach { opusPayloadTypes.add(it.groupValues[1]) }
    if (opusPayloadTypes.isEmpty()) return sdp

    var out = sdp
    opusPayloadTypes.forEach { payloadType ->
        val fmtpRegex = Regex("(a=fmtp:$payloadType )(.*)", RegexOption.IGNORE_CASE)
        out = fmtpRegex.replace(out) { match ->
            val keep =
                match.groupValues[2]
                    .split(";")
                    .filter {
                        !Regex(
                            "^(stereo|sprop-stereo|maxaveragebitrate|maxplaybackrate|ptime|minptime|usedtx|useinbandfec)=",
                            RegexOption.IGNORE_CASE,
                        ).containsMatchIn(it)
                    }
                    .joinToString(";")
            val extra =
                "stereo=1;sprop-stereo=1;maxaveragebitrate=192000;maxplaybackrate=48000;ptime=20;minptime=10;useinbandfec=1;usedtx=1"
            val result = match.groupValues[1] + extra + (if (keep.isNotEmpty()) ";$keep" else "")
            result.trimEnd(';')
        }
    }
    return out
}
