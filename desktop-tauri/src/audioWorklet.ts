// macOS: no audio MediaStreamTrackGenerator in WebKit. Interleaved stereo f32
// chunks queue here and play into a MediaStreamAudioDestinationNode.
declare const registerProcessor: (name: string, processor: unknown) => void
declare class AudioWorkletProcessor { readonly port: MessagePort }
// Older audio is dropped beyond this backlog (≈200 ms) to keep latency bounded.
const maxQueued = 48_000 * 0.2
registerProcessor('native-audio', class extends AudioWorkletProcessor {
  queue: Float32Array[] = []
  offset = 0
  queued = 0
  constructor() {
    super()
    this.port.onmessage = ({ data }) => {
      this.queue.push(data)
      this.queued += data.length / 2
      while (this.queued > maxQueued && this.queue.length > 1) {
        this.queued -= (this.queue.shift()!.length - this.offset) / 2
        this.offset = 0
      }
    }
  }
  process(_: Float32Array[][], [[left, right]]: Float32Array[][]) {
    for (let i = 0; i < left.length && this.queue.length; i++) {
      const chunk = this.queue[0]
      left[i] = chunk[this.offset]
      right[i] = chunk[this.offset + 1]
      this.offset += 2
      this.queued--
      if (this.offset >= chunk.length) { this.queue.shift(); this.offset = 0 }
    }
    return true
  }
})
