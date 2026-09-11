(() => {
  // node_modules/@tauri-apps/api/external/tslib/tslib.es6.js
  function __classPrivateFieldGet(receiver, state, kind, f) {
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a getter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot read private member from an object whose class did not declare it");
    return kind === "m" ? f : kind === "a" ? f.call(receiver) : f ? f.value : state.get(receiver);
  }
  function __classPrivateFieldSet(receiver, state, value, kind, f) {
    if (kind === "m") throw new TypeError("Private method is not writable");
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a setter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot write private member to an object whose class did not declare it");
    return kind === "a" ? f.call(receiver, value) : f ? f.value = value : state.set(receiver, value), value;
  }

  // node_modules/@tauri-apps/api/core.js
  var _Channel_onmessage;
  var _Channel_nextMessageIndex;
  var _Channel_pendingMessages;
  var _Channel_messageEndIndex;
  var _Resource_rid;
  var SERIALIZE_TO_IPC_FN = "__TAURI_TO_IPC_KEY__";
  function transformCallback(callback, once = false) {
    return window.__TAURI_INTERNALS__.transformCallback(callback, once);
  }
  var Channel = class {
    constructor(onmessage) {
      _Channel_onmessage.set(this, void 0);
      _Channel_nextMessageIndex.set(this, 0);
      _Channel_pendingMessages.set(this, []);
      _Channel_messageEndIndex.set(this, void 0);
      __classPrivateFieldSet(this, _Channel_onmessage, onmessage || (() => {
      }), "f");
      this.id = transformCallback((rawMessage) => {
        const index = rawMessage.index;
        if ("end" in rawMessage) {
          if (index == __classPrivateFieldGet(this, _Channel_nextMessageIndex, "f")) {
            this.cleanupCallback();
          } else {
            __classPrivateFieldSet(this, _Channel_messageEndIndex, index, "f");
          }
          return;
        }
        const message = rawMessage.message;
        if (index == __classPrivateFieldGet(this, _Channel_nextMessageIndex, "f")) {
          __classPrivateFieldGet(this, _Channel_onmessage, "f").call(this, message);
          __classPrivateFieldSet(this, _Channel_nextMessageIndex, __classPrivateFieldGet(this, _Channel_nextMessageIndex, "f") + 1, "f");
          while (__classPrivateFieldGet(this, _Channel_nextMessageIndex, "f") in __classPrivateFieldGet(this, _Channel_pendingMessages, "f")) {
            const message2 = __classPrivateFieldGet(this, _Channel_pendingMessages, "f")[__classPrivateFieldGet(this, _Channel_nextMessageIndex, "f")];
            __classPrivateFieldGet(this, _Channel_onmessage, "f").call(this, message2);
            delete __classPrivateFieldGet(this, _Channel_pendingMessages, "f")[__classPrivateFieldGet(this, _Channel_nextMessageIndex, "f")];
            __classPrivateFieldSet(this, _Channel_nextMessageIndex, __classPrivateFieldGet(this, _Channel_nextMessageIndex, "f") + 1, "f");
          }
          if (__classPrivateFieldGet(this, _Channel_nextMessageIndex, "f") === __classPrivateFieldGet(this, _Channel_messageEndIndex, "f")) {
            this.cleanupCallback();
          }
        } else {
          __classPrivateFieldGet(this, _Channel_pendingMessages, "f")[index] = message;
        }
      });
    }
    cleanupCallback() {
      window.__TAURI_INTERNALS__.unregisterCallback(this.id);
    }
    set onmessage(handler) {
      __classPrivateFieldSet(this, _Channel_onmessage, handler, "f");
    }
    get onmessage() {
      return __classPrivateFieldGet(this, _Channel_onmessage, "f");
    }
    [(_Channel_onmessage = /* @__PURE__ */ new WeakMap(), _Channel_nextMessageIndex = /* @__PURE__ */ new WeakMap(), _Channel_pendingMessages = /* @__PURE__ */ new WeakMap(), _Channel_messageEndIndex = /* @__PURE__ */ new WeakMap(), SERIALIZE_TO_IPC_FN)]() {
      return `__CHANNEL__:${this.id}`;
    }
    toJSON() {
      return this[SERIALIZE_TO_IPC_FN]();
    }
  };
  async function invoke(cmd, args = {}, options) {
    return window.__TAURI_INTERNALS__.invoke(cmd, args, options);
  }
  _Resource_rid = /* @__PURE__ */ new WeakMap();

  // src/desktopBridge.ts
  var frames = 480;
  var channels = 2;
  var sampleRate = 48e3;
  var bytesPerChunk = frames * channels * 4;
  var NativeCaptureError = class extends Error {
    constructor(code, message) {
      super(message);
      this.code = code;
    }
  };
  function errorFor(error) {
    const text = String(error);
    if (text.includes("generator-unavailable")) return new NativeCaptureError("generator-unavailable", "Este WebView n\xE3o oferece MediaStreamTrackGenerator.");
    if (text.includes("no-default-render-device")) return new NativeCaptureError("no-default-render-device", "Nenhum dispositivo de sa\xEDda de \xE1udio est\xE1 dispon\xEDvel.");
    if (text.includes("process-not-found")) return new NativeCaptureError("process-not-found", "O aplicativo selecionado foi fechado.");
    if (text.includes("capture")) return new NativeCaptureError("capture-failed", "N\xE3o foi poss\xEDvel iniciar a captura de \xE1udio do sistema.");
    return new NativeCaptureError("native-unavailable", "A captura de \xE1udio nativa n\xE3o est\xE1 dispon\xEDvel.");
  }
  var current;
  var metrics = { chunksReceived: 0, invalidChunks: 0, maxJitterUs: 0, totalJitterUs: 0 };
  async function startAudio(command, args) {
    if (typeof globalThis.MediaStreamTrackGenerator === "undefined" || typeof globalThis.AudioData === "undefined") {
      throw new NativeCaptureError("generator-unavailable", "MediaStreamTrackGenerator n\xE3o est\xE1 dispon\xEDvel neste WebView.");
    }
    await stopSystemAudio();
    metrics = { chunksReceived: 0, invalidChunks: 0, maxJitterUs: 0, totalJitterUs: 0 };
    const generator = new globalThis.MediaStreamTrackGenerator({ kind: "audio" });
    const writer = generator.writable.getWriter();
    let stopped = false;
    const channel = new Channel();
    let capture;
    channel.onmessage = (chunk) => {
      if (chunk.ended) {
        void capture.stop();
        return;
      }
      if (stopped) return;
      if (chunk.data.length !== bytesPerChunk) {
        metrics.invalidChunks++;
        return;
      }
      if (metrics.lastTimestampUs !== void 0) {
        const jitter = Math.abs(chunk.timestampUs - metrics.lastTimestampUs - 1e4);
        metrics.maxJitterUs = Math.max(metrics.maxJitterUs, jitter);
        metrics.totalJitterUs += jitter;
      }
      metrics.lastTimestampUs = chunk.timestampUs;
      metrics.chunksReceived++;
      const audio = new globalThis.AudioData({
        format: "f32",
        sampleRate,
        numberOfFrames: frames,
        numberOfChannels: channels,
        timestamp: chunk.timestampUs,
        data: new Uint8Array(chunk.data).buffer
      });
      writer.write(audio).catch(() => {
        void capture.stop();
      });
    };
    capture = {
      async stop() {
        if (stopped) return;
        stopped = true;
        const ownsNativeCapture = current === capture;
        if (ownsNativeCapture) current = void 0;
        channel.onmessage = () => {
        };
        if (ownsNativeCapture) await invoke("stop_audio").catch(() => {
        });
        await writer.close().catch(() => {
        });
        generator.stop();
      }
    };
    current = capture;
    try {
      await invoke(command, { ...args, onAudio: channel });
    } catch (error) {
      if (current === capture) current = void 0;
      await writer.close().catch(() => {
      });
      generator.stop();
      throw errorFor(error);
    }
    return generator;
  }
  async function startSystemAudio() {
    return startAudio("start_system_audio", {});
  }
  async function startProcessAudio(processId, includeProcessTree) {
    return startAudio("start_process_audio", { processId, includeProcessTree });
  }
  async function stopSystemAudio() {
    const active = current;
    await active?.stop();
    if (current === active) current = void 0;
  }
  globalThis.__screenshareDesktopBridge = {
    startSystemAudio: async () => {
      try {
        return await startSystemAudio();
      } catch (error) {
        throw errorFor(error);
      }
    },
    startProcessAudio: async (processId, includeProcessTree) => {
      try {
        return await startProcessAudio(processId, includeProcessTree);
      } catch (error) {
        throw errorFor(error);
      }
    },
    stopAudio: stopSystemAudio,
    enumerateSources: () => invoke("enumerate_sources"),
    captureThumbnail: (hwnd) => invoke("capture_thumbnail", { hwnd }),
    getAudioMetrics: () => ({ ...metrics })
  };
})();
