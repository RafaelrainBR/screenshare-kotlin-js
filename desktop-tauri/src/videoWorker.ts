// macOS: WebKit offers VideoTrackGenerator only in workers. The track moves to
// the page; frames arrive as `FRAME_HEADER` + NV12 buffers (capture/mac.rs).
const header = 24
const generator = new (globalThis as any).VideoTrackGenerator()
const writer = generator.writable.getWriter()
postMessage({ track: generator.track }, [generator.track] as any)
onmessage = ({ data }: MessageEvent<ArrayBuffer | 'stop'>) => {
  if (data === 'stop') { void writer.close().catch(() => {}); return }
  // Never queue behind a slow encoder: drop instead.
  if (writer.desiredSize !== null && writer.desiredSize <= 0) { postMessage({ dropped: 1 }); return }
  const view = new DataView(data)
  const frame = new VideoFrame(new Uint8Array(data, header), {
    format: 'NV12', codedWidth: view.getUint32(0, true), codedHeight: view.getUint32(4, true), timestamp: view.getFloat64(8, true),
  })
  writer.write(frame).catch(() => frame.close())
}
