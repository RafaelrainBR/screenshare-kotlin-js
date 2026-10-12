(() => {
  // src/videoWorker.ts
  var header = 24;
  var generator = new globalThis.VideoTrackGenerator();
  var writer = generator.writable.getWriter();
  postMessage({ track: generator.track }, [generator.track]);
  onmessage = ({ data }) => {
    if (data === "stop") {
      void writer.close().catch(() => {
      });
      return;
    }
    if (writer.desiredSize !== null && writer.desiredSize <= 0) {
      postMessage({ dropped: 1 });
      return;
    }
    const view = new DataView(data);
    const frame = new VideoFrame(new Uint8Array(data, header), {
      format: "NV12",
      codedWidth: view.getUint32(0, true),
      codedHeight: view.getUint32(4, true),
      timestamp: view.getFloat64(8, true)
    });
    writer.write(frame).catch(() => frame.close());
  };
})();
