import { Channel, invoke } from '@tauri-apps/api/core'

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

;(globalThis as any).__screenshareDesktopBridge = {
  startSystemAudio: async () => {
    try { return await startSystemAudio() } catch (error) { throw errorFor(error) }
  },
  startProcessAudio: async (processId: number, includeProcessTree: boolean) => { try { return await startProcessAudio(processId, includeProcessTree) } catch (error) { throw errorFor(error) } },
  stopAudio: stopSystemAudio,
  enumerateSources: () => invoke<NativeSource[]>('enumerate_sources'),
  captureThumbnail: (hwnd: number) => invoke('capture_thumbnail', { hwnd }),
  getAudioMetrics: () => ({ ...metrics }),
}
