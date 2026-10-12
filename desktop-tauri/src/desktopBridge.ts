import { Channel, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

const frames = 480
const channels = 2
const sampleRate = 48_000
const bytesPerChunk = frames * channels * 4

type AudioChunk = { timestampUs: number; data: number[]; ended?: boolean }
type NativeSource = { id: string; title: string; hwnd: number; pid: number; rect: [number, number, number, number]; kind: 'screen' | 'window' }
type AudioCaptureMetrics = {
  chunksReceived: number
  invalidChunks: number
  maxJitterUs: number
  totalJitterUs: number
  lastTimestampUs?: number
}

class NativeCaptureError extends Error {
  constructor(readonly code: string, message: string) { super(message) }
}

function errorFor(error: unknown): NativeCaptureError {
  const text = String(error)
  if (text.includes('generator-unavailable')) return new NativeCaptureError('generator-unavailable', 'Este WebView não oferece MediaStreamTrackGenerator.')
  if (text.includes('no-default-render-device')) return new NativeCaptureError('no-default-render-device', 'Nenhum dispositivo de saída de áudio está disponível.')
  if (text.includes('process-not-found')) return new NativeCaptureError('process-not-found', 'O aplicativo selecionado foi fechado.')
  if (text.includes('capture')) return new NativeCaptureError('capture-failed', 'Não foi possível iniciar a captura de áudio do sistema.')
  return new NativeCaptureError('native-unavailable', 'A captura de áudio nativa não está disponível.')
}

type NativeCapture = { stop: () => Promise<void>; track?: unknown }
let current: NativeCapture | undefined
let metrics: AudioCaptureMetrics = { chunksReceived: 0, invalidChunks: 0, maxJitterUs: 0, totalJitterUs: 0 }

const isMac = navigator.userAgent.includes('Mac')
const hasGenerator = typeof (globalThis as any).MediaStreamTrackGenerator !== 'undefined'

type AudioSink = { track: MediaStreamTrack; write: (pcm: ArrayBuffer, timestampUs: number) => Promise<void>; close: () => Promise<void> }

// WebView2 has an audio MediaStreamTrackGenerator; WebKit (macOS) plays the PCM
// through an AudioWorklet into a MediaStreamAudioDestinationNode instead.
async function audioSink(): Promise<AudioSink> {
  const g = globalThis as any
  if (hasGenerator && typeof g.AudioData !== 'undefined') {
    const generator = new g.MediaStreamTrackGenerator({ kind: 'audio' })
    const writer = generator.writable.getWriter()
    return {
      track: generator,
      write: (data, timestamp) => writer.write(new g.AudioData({
        format: 'f32', sampleRate, numberOfFrames: frames, numberOfChannels: channels, timestamp, data,
      })),
      async close() { await writer.close().catch(() => {}); generator.stop() },
    }
  }
  if (!isMac || typeof g.AudioWorkletNode === 'undefined') {
    throw new NativeCaptureError('generator-unavailable', 'Este WebView não oferece um gerador de áudio.')
  }
  const context = new AudioContext({ sampleRate })
  await context.audioWorklet.addModule('audioWorklet.js')
  const node = new AudioWorkletNode(context, 'native-audio', { numberOfInputs: 0, outputChannelCount: [channels] })
  const destination = context.createMediaStreamDestination()
  node.connect(destination)
  void context.resume()
  const track = destination.stream.getAudioTracks()[0]
  return {
    track,
    async write(data) { const pcm = new Float32Array(data); node.port.postMessage(pcm, [pcm.buffer]) },
    async close() { track.stop(); await context.close().catch(() => {}) },
  }
}

async function startAudio(command: 'start_system_audio' | 'start_process_audio', args: Record<string, unknown>): Promise<MediaStreamTrack> {
  await stopSystemAudio()
  metrics = { chunksReceived: 0, invalidChunks: 0, maxJitterUs: 0, totalJitterUs: 0 }
  const sink = await audioSink()
  let stopped = false
  const channel = new Channel<AudioChunk>()
  let capture: NativeCapture
  channel.onmessage = (chunk) => {
    if (chunk.ended) { void capture.stop(); return }
    if (stopped) return
    if (chunk.data.length !== bytesPerChunk) { metrics.invalidChunks++; return }
    if (metrics.lastTimestampUs !== undefined) {
      const jitter = Math.abs(chunk.timestampUs - metrics.lastTimestampUs - 10_000)
      metrics.maxJitterUs = Math.max(metrics.maxJitterUs, jitter)
      metrics.totalJitterUs += jitter
    }
    metrics.lastTimestampUs = chunk.timestampUs
    metrics.chunksReceived++
    sink.write(new Uint8Array(chunk.data).buffer, chunk.timestampUs).catch(() => { void capture.stop() })
  }
  capture = {
    track: sink.track,
    async stop() {
      if (stopped) return
      stopped = true
      // An old channel may deliver its terminal marker after a replacement has
      // installed a new native worker. Only the owner may stop Rust globally;
      // otherwise that late marker would terminate the replacement capture.
      const ownsNativeCapture = current === capture
      if (ownsNativeCapture) current = undefined
      channel.onmessage = () => {}
      if (ownsNativeCapture) await invoke('stop_audio').catch(() => {})
      await sink.close()
    },
  }
  // Install the ownership record before invoking Rust: a fast terminal marker
  // must close this generator, never a later capture.
  current = capture
  try {
    await invoke(command, { ...args, onAudio: channel })
  } catch (error) {
    if (current === capture) current = undefined
    await sink.close()
    throw errorFor(error)
  }
  return sink.track
}
async function startSystemAudio(): Promise<MediaStreamTrack> { return startAudio('start_system_audio', {}) }
async function startProcessAudio(processId: number, includeProcessTree: boolean): Promise<MediaStreamTrack> { return startAudio('start_process_audio', { processId, includeProcessTree }) }

// With a track, stops only the capture that produced it: a late stop for a
// replaced share must not end its replacement.
async function stopSystemAudio(track?: unknown): Promise<void> {
  const active = current
  if (track != null && active?.track !== track) return
  await active?.stop()
  // `capture.stop()` clears its own ownership. Keep this fallback for a
  // generator that was never fully installed.
  if (current === active) current = undefined
}

type VideoRequest = { hwnd?: number; rect?: [number, number, number, number]; fps: number }
type VideoMetrics = { received: number; written: number; droppedWriter: number; latencyUs: number[] }
const sharedHeader = 16
let currentVideo: NativeCapture | undefined
let videoMetrics: VideoMetrics = { received: 0, written: 0, droppedWriter: 0, latencyUs: [] }
// Shared memory slots stay mapped for the whole capture; frames only name a slot.
const sharedSlots = new Map<number, { generation: number; data: ArrayBuffer }>()
function releaseSharedSlots(keepGeneration?: number) {
  for (const [slot, buffer] of sharedSlots) {
    if (buffer.generation === keepGeneration) continue
    sharedSlots.delete(slot)
    ;(globalThis as any).chrome.webview.releaseBuffer(buffer.data)
  }
}
;(globalThis as any).chrome?.webview?.addEventListener('sharedbufferreceived', (event: any) => {
  const { generation, slot } = event.additionalData
  releaseSharedSlots(generation)
  const data: ArrayBuffer = event.getBuffer()
  // A frame announced before its slot arrived was skipped: free the slot again.
  new Uint8Array(data)[0] = 0
  sharedSlots.set(slot, { generation, data })
})

// `onEnded` reports a capture that ended on its own (e.g. 'window-closed'): a
// generator track stopped from script fires no `ended` event of its own.
async function startVideo(request: VideoRequest, onEnded?: (error: string) => void): Promise<MediaStreamTrack> {
  await stopVideo()
  videoMetrics = { received: 0, written: 0, droppedWriter: 0, latencyUs: [] }
  const channel = new Channel<any>()
  let stopped = false
  let capture: NativeCapture
  const sink = await videoSink(() => { void capture.stop() })
  channel.onmessage = (message) => {
    if (message?.ended) {
      void capture.stop()
      onEnded?.(message.error ?? '')
    } else if (!stopped) {
      sink.frame(message)
    }
  }
  capture = {
    track: sink.track,
    async stop() {
      if (stopped) return
      stopped = true
      const owns = currentVideo === capture
      if (owns) currentVideo = undefined
      channel.onmessage = () => {}
      if (owns) await invoke('stop_video_capture').catch(() => {})
      if (owns) releaseSharedSlots()
      await sink.close()
    },
  }
  currentVideo = capture
  try {
    await invoke('start_video_capture', { request, onFrame: channel })
  } catch (error) {
    await capture.stop()
    const text = String(error)
    if (text.includes('screen-recording-denied')) throw new NativeCaptureError('screen-recording-denied', screenRecordingDenied)
    throw new NativeCaptureError(text.includes('shared-buffer') ? 'shared-buffer-unavailable' : 'capture-failed', 'Não foi possível iniciar a captura de vídeo.')
  }
  return sink.track
}

const screenRecordingDenied = 'Permita a Gravação de Tela para o Screenshare em Ajustes do Sistema › Privacidade e Segurança e abra o app de novo.'

type VideoSink = { track: MediaStreamTrack; frame: (message: any) => void; close: () => Promise<void> }

function noteFrame(wallUs: number) {
  videoMetrics.received++
  // Bounded sample window for latency percentiles.
  if (videoMetrics.latencyUs.push(Date.now() * 1000 - wallUs) > 600) videoMetrics.latencyUs.shift()
}

// Windows: BGRA frames in WebView2 shared memory feed a MediaStreamTrackGenerator.
// macOS: raw NV12 channel messages feed a VideoTrackGenerator in a worker.
async function videoSink(onFailed: () => void): Promise<VideoSink> {
  const g = globalThis as any
  if (typeof g.VideoFrame === 'undefined' || (!hasGenerator && !isMac)) {
    throw new NativeCaptureError('generator-unavailable', 'Este WebView não oferece um gerador de vídeo.')
  }
  if (hasGenerator) {
    const generator = new g.MediaStreamTrackGenerator({ kind: 'video' })
    const writer = generator.writable.getWriter()
    return {
      track: generator,
      frame(message) {
        const buffer = sharedSlots.get(message?.slot)
        if (buffer === undefined || buffer.generation !== message.generation) return
        try {
          noteFrame(message.wallUs)
          // Never queue behind a slow encoder: drop instead.
          if (writer.desiredSize !== null && writer.desiredSize <= 0) { videoMetrics.droppedWriter++; return }
          const data = new Uint8Array(buffer.data, sharedHeader, message.width * message.height * 4)
          const frame = new g.VideoFrame(data, { format: 'BGRA', codedWidth: message.width, codedHeight: message.height, timestamp: message.timestampUs })
          videoMetrics.written++
          writer.write(frame).catch(() => { frame.close(); onFailed() })
        } finally {
          // VideoFrame copied the pixels: hand the slot back to the capture thread.
          new Uint8Array(buffer.data)[0] = 0
        }
      },
      async close() { await writer.close().catch(() => {}); generator.stop() },
    }
  }
  const worker = new Worker('videoWorker.js')
  const track = await new Promise<MediaStreamTrack>((resolve, reject) => {
    worker.onmessage = ({ data }) => resolve(data.track)
    worker.onerror = () => reject(new NativeCaptureError('generator-unavailable', 'Este WebView não oferece um gerador de vídeo.'))
  })
  worker.onmessage = ({ data }) => { if (data?.dropped) { videoMetrics.droppedWriter++; videoMetrics.written-- } }
  return {
    track,
    frame(message) {
      if (!(message instanceof ArrayBuffer)) return
      noteFrame(new DataView(message).getFloat64(16, true))
      videoMetrics.written++
      worker.postMessage(message, [message])
    },
    async close() { track.stop(); worker.terminate() },
  }
}

async function stopVideo(track?: unknown): Promise<void> {
  if (track != null && currentVideo?.track !== track) return
  await currentVideo?.stop()
}

function videoSummary() {
  const sorted = [...videoMetrics.latencyUs].sort((a, b) => a - b)
  const at = (q: number) => sorted.length ? Math.round(sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))]) : null
  return { received: videoMetrics.received, written: videoMetrics.written, droppedWriter: videoMetrics.droppedWriter, latencyUsP50: at(0.5), latencyUsP95: at(0.95), latencyUsMax: at(1) }
}

;(globalThis as any).__screenshareDesktopBridge = {
  startSystemAudio: async () => {
    try { return await startSystemAudio() } catch (error) { throw errorFor(error) }
  },
  startProcessAudio: async (processId: number, includeProcessTree: boolean) => { try { return await startProcessAudio(processId, includeProcessTree) } catch (error) { throw errorFor(error) } },
  stopAudio: stopSystemAudio,
  enumerateSources: () => invoke<NativeSource[]>('enumerate_sources').catch((error) => {
    throw String(error).includes('screen-recording-denied') ? new NativeCaptureError('screen-recording-denied', screenRecordingDenied) : error
  }),
  captureThumbnail: (hwnd: number, rect?: [number, number, number, number]) => invoke('capture_thumbnail', { hwnd, rect }),
  getAudioMetrics: () => ({ ...metrics }),
  startVideoCapture: startVideo,
  stopVideoCapture: stopVideo,
  getVideoMetrics: async () => ({ js: videoSummary(), native: isMac ? null : await invoke('video_capture_metrics') }),
  // Native capture commands (Windows and macOS), not the common shell API.
  isAvailable: navigator.userAgent.includes('Windows') || isMac,
  // A native audio track can be built: a generator (WebView2) or an AudioWorklet (WebKit).
  nativeAudio: (hasGenerator && typeof (globalThis as any).AudioData !== 'undefined') || (isMac && typeof (globalThis as any).AudioWorkletNode !== 'undefined'),
  mediaAvailable: true,
  autoStartEnabled: () => invoke<boolean>('plugin:autostart|is_enabled'),
  setAutoStart: (enabled: boolean) => invoke(enabled ? 'plugin:autostart|enable' : 'plugin:autostart|disable'),
  hideMainWindow: () => invoke('hide_main_window'),
  showMainWindow: () => invoke('show_main_window'),
  quitApp: () => invoke('quit_app'),
  setTrayState: (kind: string, active: boolean, participants?: number, detail?: string) =>
    invoke('set_tray_state', { kind, active, participants, detail }),
}

void listen<string>('tray-action', ({ payload }) => {
  window.dispatchEvent(new CustomEvent('screenshare:tray-action', { detail: payload }))
})

void listen<boolean>('session-media-available', ({ payload }) => {
  (globalThis as any).__screenshareDesktopBridge.mediaAvailable = payload
  window.dispatchEvent(new CustomEvent('screenshare:session-media-available', { detail: payload }))
})

void listen<string>('viewer-mode', ({ payload }) => {
  window.dispatchEvent(new CustomEvent('screenshare:viewer-mode', { detail: payload }))
})
