@file:OptIn(kotlin.js.ExperimentalWasmJsInterop::class)

package screenshare.clientkmp.services

import kotlin.js.JsAny

// ─── RTCPeerConnection ────────────────────────────────────────────────────────

@JsFun("() => new RTCPeerConnection({ iceServers: [{ urls: 'stun:stun.l.google.com:19302' }] })")
internal external fun jsNewPeerConnection(): JsAny

/**
 * Creates an offer, waits for ICE gathering to complete, then pushes
 * { type, sdp } (or { error:true }) to pc.__sdpQueue.
 */
@JsFun(
    """(pc) => {
    pc.__sdpQueue = pc.__sdpQueue || [];
    pc.createOffer()
        .then(o => pc.setLocalDescription(o))
        .then(() => new Promise(res => {
            if (pc.iceGatheringState === 'complete') { res(pc.localDescription); return; }
            pc.onicegatheringstatechange = () => {
                if (pc.iceGatheringState === 'complete') res(pc.localDescription);
            };
        }))
        .then(desc => pc.__sdpQueue.push({ type: desc.type, sdp: desc.sdp }))
        .catch(() => pc.__sdpQueue.push({ error: true }));
}""",
)
internal external fun jsCreateOffer(pc: JsAny)

/**
 * Sets remote offer, creates answer, waits for ICE, then pushes
 * { type, sdp } (or { error:true }) to pc.__sdpQueue.
 */
@JsFun(
    """(pc, type, sdp) => {
    pc.__sdpQueue = pc.__sdpQueue || [];
    pc.setRemoteDescription({ type: type, sdp: sdp })
        .then(() => pc.createAnswer())
        .then(a => pc.setLocalDescription(a))
        .then(() => new Promise(res => {
            if (pc.iceGatheringState === 'complete') { res(pc.localDescription); return; }
            pc.onicegatheringstatechange = () => {
                if (pc.iceGatheringState === 'complete') res(pc.localDescription);
            };
        }))
        .then(desc => pc.__sdpQueue.push({ type: desc.type, sdp: desc.sdp }))
        .catch(() => pc.__sdpQueue.push({ error: true }));
}""",
)
internal external fun jsSetRemoteAndAnswer(
    pc: JsAny,
    type: String,
    sdp: String,
)

/** Pops the first SDP result from pc.__sdpQueue, or returns null if none. */
@JsFun("(pc) => (pc.__sdpQueue && pc.__sdpQueue.length) ? pc.__sdpQueue.shift() : null")
internal external fun jsPopSdpResult(pc: JsAny): JsAny?

@JsFun("(r) => !!r.error")
internal external fun jsSdpResultIsError(r: JsAny): Boolean

@JsFun("(r) => r.type || ''")
internal external fun jsSdpType(r: JsAny): String

@JsFun("(r) => r.sdp || ''")
internal external fun jsSdpContent(r: JsAny): String

/** Sets remote description (answer) — fire-and-forget. */
@JsFun("(pc, type, sdp) => pc.setRemoteDescription({ type: type, sdp: sdp }).catch(() => {})")
internal external fun jsSetRemoteAnswer(
    pc: JsAny,
    type: String,
    sdp: String,
)

/** Adds an ICE candidate — fire-and-forget. */
@JsFun("(pc, json) => pc.addIceCandidate(JSON.parse(json)).catch(() => {})")
internal external fun jsAddIceCandidate(
    pc: JsAny,
    candidateJson: String,
)

@JsFun("(pc) => pc.close()")
internal external fun jsPcClose(pc: JsAny)

// ─── Track management ────────────────────────────────────────────────────────

@JsFun("(pc, track, stream) => pc.addTrack(track, stream)")
internal external fun jsAddTrack(
    pc: JsAny,
    track: JsAny,
    stream: JsAny,
)

@JsFun("(pc, trackId) => pc.getSenders().some(s => s.track && s.track.id === trackId)")
internal external fun jsHasTrack(
    pc: JsAny,
    trackId: String,
): Boolean

@JsFun(
    """(pc) => {
    pc.__trackQueue = [];
    pc.ontrack = e => pc.__trackQueue.push({ isVideo: e.track.kind === 'video', stream: e.streams[0] });
}""",
)
internal external fun jsSetupTrackQueue(pc: JsAny)

@JsFun("(pc) => (pc.__trackQueue && pc.__trackQueue.length) ? pc.__trackQueue.shift() : null")
internal external fun jsPopTrackEvent(pc: JsAny): JsAny?

@JsFun("(e) => !!e.isVideo")
internal external fun jsEventIsVideo(e: JsAny): Boolean

@JsFun("(e) => e.stream")
internal external fun jsEventStream(e: JsAny): JsAny

// ─── Stream / Track helpers ───────────────────────────────────────────────────

@JsFun("(stream) => stream.getTracks()")
internal external fun jsGetAllTracks(stream: JsAny): JsAny

@JsFun("(arr) => arr.length")
internal external fun jsArrayLength(arr: JsAny): Int

@JsFun("(arr, i) => arr[i]")
internal external fun jsArrayGet(
    arr: JsAny,
    i: Int,
): JsAny

@JsFun("(track) => track.id")
internal external fun jsTrackId(track: JsAny): String

@JsFun("(track, enabled) => { track.enabled = !!enabled; }")
internal external fun jsSetTrackEnabled(
    track: JsAny,
    enabled: Boolean,
)

@JsFun("(track) => track.stop()")
internal external fun jsTrackStop(track: JsAny)

// ─── Media capture — queue-based (no Promise return to Kotlin) ────────────────

@JsFun(
    """() => {
    globalThis.__micQueue = globalThis.__micQueue || [];
    navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: false, sampleRate: 48000 }
    })
    .then(s  => globalThis.__micQueue.push({ ok: true,  stream: s }))
    .catch(() => globalThis.__micQueue.push({ ok: false, stream: null }));
}""",
)
internal external fun jsRequestUserMedia()

@JsFun(
    """() => {
    globalThis.__screenQueue = globalThis.__screenQueue || [];
    navigator.mediaDevices.getDisplayMedia({
        video: { cursor: 'always', frameRate: { ideal: 30 }, width: { ideal: 1920 }, height: { ideal: 1080 } },
        audio: false
    })
    .then(s  => globalThis.__screenQueue.push({ ok: true,  stream: s }))
    .catch(() => globalThis.__screenQueue.push({ ok: false, stream: null }));
}""",
)
internal external fun jsRequestDisplayMedia()

/** Pops from the named globalThis queue (e.g. "__micQueue"), or returns null. */
@JsFun("(q) => { const a = globalThis[q]; return (a && a.length) ? a.shift() : null; }")
internal external fun jsPopMediaResult(queueName: String): JsAny?

@JsFun("(r) => !!r.ok")
internal external fun jsMediaResultOk(r: JsAny): Boolean

@JsFun("(r) => r.stream")
internal external fun jsMediaResultStream(r: JsAny): JsAny

// ─── Audio level — polling via JS global queue ────────────────────────────────

@JsFun(
    """(stream, threshold, socketId) => {
    const ctx = new AudioContext();
    const src = ctx.createMediaStreamSource(stream);
    const ana = ctx.createAnalyser(); ana.fftSize = 512;
    const buf = new Uint8Array(ana.frequencyBinCount);
    src.connect(ana);
    let speaking = false;
    globalThis.__ktSpeakQueue = globalThis.__ktSpeakQueue || [];
    const handle = { ctx: ctx };
    const loop = () => {
        ana.getByteFrequencyData(buf);
        const vol = buf.reduce((s, v) => s + v, 0) / buf.length;
        const isSpeaking = vol > threshold;
        if (isSpeaking !== speaking) {
            speaking = isSpeaking;
            globalThis.__ktSpeakQueue.push({ id: socketId, speaking: isSpeaking });
        }
        handle.__raf = requestAnimationFrame(loop);
    };
    handle.__raf = requestAnimationFrame(loop);
    return handle;
}""",
)
internal external fun jsStartAudioMonitor(
    stream: JsAny,
    threshold: Int,
    socketId: String,
): JsAny

@JsFun("(h) => { cancelAnimationFrame(h.__raf); if (h.ctx) h.ctx.close(); }")
internal external fun jsStopAudioMonitor(h: JsAny)

@JsFun(
    "() => (globalThis.__ktSpeakQueue && globalThis.__ktSpeakQueue.length) ? globalThis.__ktSpeakQueue.shift() : null",
)
internal external fun jsPopSpeakEvent(): JsAny?

@JsFun("(e) => e.id || ''")
internal external fun jsSpeakId(e: JsAny): String

@JsFun("(e) => !!e.speaking")
internal external fun jsSpeakFlag(e: JsAny): Boolean
