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

let current: { stop: () => Promise<void> } | undefined
let metrics: AudioCaptureMetrics = { chunksReceived: 0, invalidChunks: 0, maxJitterUs: 0, totalJitterUs: 0 }

async function startAudio(command: 'start_system_audio' | 'start_process_audio', args: Record<string, unknown>): Promise<MediaStreamTrack> {
  if (typeof (globalThis as any).MediaStreamTrackGenerator === 'undefined' || typeof (globalThis as any).AudioData === 'undefined') {
    throw new NativeCaptureError('generator-unavailable', 'MediaStreamTrackGenerator não está disponível neste WebView.')
  }
  await stopSystemAudio()
  metrics = { chunksReceived: 0, invalidChunks: 0, maxJitterUs: 0, totalJitterUs: 0 }
  const generator = new (globalThis as any).MediaStreamTrackGenerator({ kind: 'audio' })
  const writer = generator.writable.getWriter()
  let stopped = false
  const channel = new Channel<AudioChunk>()
  let capture: { stop: () => Promise<void> }
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
    const audio = new (globalThis as any).AudioData({
      format: 'f32', sampleRate, numberOfFrames: frames, numberOfChannels: channels,
      timestamp: chunk.timestampUs, data: new Uint8Array(chunk.data).buffer,
    })
    writer.write(audio).catch(() => { void capture.stop() })
  }
  capture = {
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
      await writer.close().catch(() => {})
      generator.stop()
    },
  }
  // Install the ownership record before invoking Rust: a fast terminal marker
  // must close this generator, never a later capture.
  current = capture
  try {
    await invoke(command, { ...args, onAudio: channel })
  } catch (error) {
    if (current === capture) current = undefined
    await writer.close().catch(() => {})
    generator.stop()
    throw errorFor(error)
  }
  return generator as MediaStreamTrack
}
async function startSystemAudio(): Promise<MediaStreamTrack> { return startAudio('start_system_audio', {}) }
async function startProcessAudio(processId: number, includeProcessTree: boolean): Promise<MediaStreamTrack> { return startAudio('start_process_audio', { processId, includeProcessTree }) }

async function stopSystemAudio(): Promise<void> {
  const active = current
  await active?.stop()
  // `capture.stop()` clears its own ownership. Keep this fallback for a
  // generator that was never fully installed.
  if (current === active) current = undefined
}

type VideoRequest = { hwnd?: number; rect?: [number, number, number, number]; fps: number }
type VideoMetrics = { received: number; written: number; droppedWriter: number; latencyUs: number[] }
const sharedHeader = 16
let currentVideo: { stop: () => Promise<void> } | undefined
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

async function startVideo(request: VideoRequest): Promise<MediaStreamTrack> {
  const g = globalThis as any
  if (typeof g.MediaStreamTrackGenerator === 'undefined' || typeof g.VideoFrame === 'undefined') {
    throw new NativeCaptureError('generator-unavailable', 'MediaStreamTrackGenerator não está disponível neste WebView.')
  }
  await stopVideo()
  videoMetrics = { received: 0, written: 0, droppedWriter: 0, latencyUs: [] }
  const generator = new g.MediaStreamTrackGenerator({ kind: 'video' })
  const writer = generator.writable.getWriter()
  const channel = new Channel<any>()
  let stopped = false
  let capture: { stop: () => Promise<void> }
  const write = (data: Uint8Array, width: number, height: number, timestampUs: number, wallUs: number) => {
    videoMetrics.received++
    // Bounded sample window for latency percentiles.
    if (videoMetrics.latencyUs.push(Date.now() * 1000 - wallUs) > 600) videoMetrics.latencyUs.shift()
    if (stopped) return
    // Never queue behind a slow encoder: drop instead.
    if (writer.desiredSize !== null && writer.desiredSize <= 0) { videoMetrics.droppedWriter++; return }
    const frame = new g.VideoFrame(data, { format: 'BGRA', codedWidth: width, codedHeight: height, timestamp: timestampUs })
    videoMetrics.written++
    writer.write(frame).catch(() => { frame.close(); void capture.stop() })
  }
  channel.onmessage = (message) => {
    if (message?.slot !== undefined) {
      const buffer = sharedSlots.get(message.slot)
      if (buffer?.generation !== message.generation) return
      try {
        write(new Uint8Array(buffer.data, sharedHeader, message.width * message.height * 4), message.width, message.height, message.timestampUs, message.wallUs)
      } finally {
        // VideoFrame copied the pixels: hand the slot back to the capture thread.
        new Uint8Array(buffer.data)[0] = 0
      }
    } else if (message?.ended) {
      void capture.stop()
    }
  }
  capture = {
    async stop() {
      if (stopped) return
      stopped = true
      const owns = currentVideo === capture
      if (owns) currentVideo = undefined
      channel.onmessage = () => {}
      if (owns) await invoke('stop_video_capture').catch(() => {})
      if (owns) releaseSharedSlots()
      await writer.close().catch(() => {})
      generator.stop()
    },
  }
  currentVideo = capture
  try {
    await invoke('start_video_capture', { request, onFrame: channel })
  } catch (error) {
    await capture.stop()
    throw new NativeCaptureError(String(error).includes('shared-buffer') ? 'shared-buffer-unavailable' : 'capture-failed', 'Não foi possível iniciar a captura de vídeo.')
  }
  return generator as MediaStreamTrack
}

async function stopVideo(): Promise<void> {
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
  enumerateSources: () => invoke<NativeSource[]>('enumerate_sources'),
  captureThumbnail: (hwnd: number) => invoke('capture_thumbnail', { hwnd }),
  getAudioMetrics: () => ({ ...metrics }),
  startVideoCapture: startVideo,
  stopVideoCapture: stopVideo,
  getVideoMetrics: async () => ({ js: videoSummary(), native: await invoke('video_capture_metrics') }),
  // This capability covers Windows capture commands, not the common shell API.
  isAvailable: navigator.userAgent.includes('Windows'),
  mediaAvailable: true,
  autoStartEnabled: () => invoke<boolean>('plugin:autostart|is_enabled'),
  setAutoStart: (enabled: boolean) => invoke(enabled ? 'plugin:autostart|enable' : 'plugin:autostart|disable'),
  hideMainWindow: () => invoke('hide_main_window'),
  showMainWindow: () => invoke('show_main_window'),
  quitApp: () => invoke('quit_app'),
  setTrayState: (kind: string, active: boolean, participants?: number) =>
    invoke('set_tray_state', { kind, active, participants }),
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
