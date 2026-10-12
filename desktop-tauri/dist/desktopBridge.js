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
    /**
     * The handler called for every message sent by the Rust side of this channel.
     *
     * Assigning a new handler replaces the previous one; messages that arrived
     * before a handler was set are not replayed, so set it (or pass it to the
     * constructor) before sending the channel to the backend.
     */
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
  var Resource = class {
    get rid() {
      return __classPrivateFieldGet(this, _Resource_rid, "f");
    }
    constructor(rid) {
      _Resource_rid.set(this, void 0);
      __classPrivateFieldSet(this, _Resource_rid, rid, "f");
    }
    /**
     * Destroys and cleans up this resource from memory.
     * **You should not call any method on this object anymore and should drop any reference to it.**
     *
     * @remarks Uses the `core:resources:allow-close` permission, which is part of
     * the `core:resources:default` permission set enabled by default.
     */
    async close() {
      return invoke("plugin:resources|close", {
        rid: this.rid
      });
    }
    async [(_Resource_rid = /* @__PURE__ */ new WeakMap(), Symbol.asyncDispose)]() {
      await this.close();
    }
  };

  // node_modules/@tauri-apps/api/event.js
  var TauriEvent;
  (function(TauriEvent2) {
    TauriEvent2["WINDOW_RESIZED"] = "tauri://resize";
    TauriEvent2["WINDOW_MOVED"] = "tauri://move";
    TauriEvent2["WINDOW_CLOSE_REQUESTED"] = "tauri://close-requested";
    TauriEvent2["WINDOW_DESTROYED"] = "tauri://destroyed";
    TauriEvent2["WINDOW_FOCUS"] = "tauri://focus";
    TauriEvent2["WINDOW_BLUR"] = "tauri://blur";
    TauriEvent2["WINDOW_SCALE_FACTOR_CHANGED"] = "tauri://scale-change";
    TauriEvent2["WINDOW_THEME_CHANGED"] = "tauri://theme-changed";
    TauriEvent2["WINDOW_CREATED"] = "tauri://window-created";
    TauriEvent2["WINDOW_SUSPENDED"] = "tauri://suspended";
    TauriEvent2["WINDOW_RESUMED"] = "tauri://resumed";
    TauriEvent2["WEBVIEW_CREATED"] = "tauri://webview-created";
    TauriEvent2["DRAG_ENTER"] = "tauri://drag-enter";
    TauriEvent2["DRAG_OVER"] = "tauri://drag-over";
    TauriEvent2["DRAG_DROP"] = "tauri://drag-drop";
    TauriEvent2["DRAG_LEAVE"] = "tauri://drag-leave";
  })(TauriEvent || (TauriEvent = {}));
  async function _unlisten(event, eventId) {
    window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(event, eventId);
    await invoke("plugin:event|unlisten", {
      event,
      eventId
    });
  }
  async function listen(event, handler, options) {
    var _a;
    const target = typeof (options === null || options === void 0 ? void 0 : options.target) === "string" ? { kind: "AnyLabel", label: options.target } : (_a = options === null || options === void 0 ? void 0 : options.target) !== null && _a !== void 0 ? _a : { kind: "Any" };
    return invoke("plugin:event|listen", {
      event,
      target,
      handler: transformCallback(handler)
    }).then((eventId) => {
      return async () => _unlisten(event, eventId);
    });
  }

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
  var isMac = navigator.userAgent.includes("Mac");
  var hasGenerator = typeof globalThis.MediaStreamTrackGenerator !== "undefined";
  async function audioSink() {
    const g = globalThis;
    if (hasGenerator && typeof g.AudioData !== "undefined") {
      const generator = new g.MediaStreamTrackGenerator({ kind: "audio" });
      const writer = generator.writable.getWriter();
      return {
        track: generator,
        write: (data, timestamp) => writer.write(new g.AudioData({
          format: "f32",
          sampleRate,
          numberOfFrames: frames,
          numberOfChannels: channels,
          timestamp,
          data
        })),
        async close() {
          await writer.close().catch(() => {
          });
          generator.stop();
        }
      };
    }
    if (!isMac || typeof g.AudioWorkletNode === "undefined") {
      throw new NativeCaptureError("generator-unavailable", "Este WebView n\xE3o oferece um gerador de \xE1udio.");
    }
    const context = new AudioContext({ sampleRate });
    await context.audioWorklet.addModule("audioWorklet.js");
    const node = new AudioWorkletNode(context, "native-audio", { numberOfInputs: 0, outputChannelCount: [channels] });
    const destination = context.createMediaStreamDestination();
    node.connect(destination);
    void context.resume();
    const track = destination.stream.getAudioTracks()[0];
    return {
      track,
      async write(data) {
        const pcm = new Float32Array(data);
        node.port.postMessage(pcm, [pcm.buffer]);
      },
      async close() {
        track.stop();
        await context.close().catch(() => {
        });
      }
    };
  }
  async function startAudio(command, args) {
    await stopSystemAudio();
    metrics = { chunksReceived: 0, invalidChunks: 0, maxJitterUs: 0, totalJitterUs: 0 };
    const sink = await audioSink();
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
      sink.write(new Uint8Array(chunk.data).buffer, chunk.timestampUs).catch(() => {
        void capture.stop();
      });
    };
    capture = {
      track: sink.track,
      async stop() {
        if (stopped) return;
        stopped = true;
        const ownsNativeCapture = current === capture;
        if (ownsNativeCapture) current = void 0;
        channel.onmessage = () => {
        };
        if (ownsNativeCapture) await invoke("stop_audio").catch(() => {
        });
        await sink.close();
      }
    };
    current = capture;
    try {
      await invoke(command, { ...args, onAudio: channel });
    } catch (error) {
      if (current === capture) current = void 0;
      await sink.close();
      throw errorFor(error);
    }
    return sink.track;
  }
  async function startSystemAudio() {
    return startAudio("start_system_audio", {});
  }
  async function startProcessAudio(processId, includeProcessTree) {
    return startAudio("start_process_audio", { processId, includeProcessTree });
  }
  async function stopSystemAudio(track) {
    const active = current;
    if (track != null && active?.track !== track) return;
    await active?.stop();
    if (current === active) current = void 0;
  }
  var sharedHeader = 16;
  var currentVideo;
  var videoMetrics = { received: 0, written: 0, droppedWriter: 0, latencyUs: [] };
  var sharedSlots = /* @__PURE__ */ new Map();
  function releaseSharedSlots(keepGeneration) {
    for (const [slot, buffer] of sharedSlots) {
      if (buffer.generation === keepGeneration) continue;
      sharedSlots.delete(slot);
      globalThis.chrome.webview.releaseBuffer(buffer.data);
    }
  }
  globalThis.chrome?.webview?.addEventListener("sharedbufferreceived", (event) => {
    const { generation, slot } = event.additionalData;
    releaseSharedSlots(generation);
    const data = event.getBuffer();
    new Uint8Array(data)[0] = 0;
    sharedSlots.set(slot, { generation, data });
  });
  async function startVideo(request, onEnded) {
    await stopVideo();
    videoMetrics = { received: 0, written: 0, droppedWriter: 0, latencyUs: [] };
    const channel = new Channel();
    let stopped = false;
    let capture;
    const sink = await videoSink(() => {
      void capture.stop();
    });
    channel.onmessage = (message) => {
      if (message?.ended) {
        void capture.stop();
        onEnded?.(message.error ?? "");
      } else if (!stopped) {
        sink.frame(message);
      }
    };
    capture = {
      track: sink.track,
      async stop() {
        if (stopped) return;
        stopped = true;
        const owns = currentVideo === capture;
        if (owns) currentVideo = void 0;
        channel.onmessage = () => {
        };
        if (owns) await invoke("stop_video_capture").catch(() => {
        });
        if (owns) releaseSharedSlots();
        await sink.close();
      }
    };
    currentVideo = capture;
    try {
      await invoke("start_video_capture", { request, onFrame: channel });
    } catch (error) {
      await capture.stop();
      const text = String(error);
      if (text.includes("screen-recording-denied")) throw new NativeCaptureError("screen-recording-denied", screenRecordingDenied);
      throw new NativeCaptureError(text.includes("shared-buffer") ? "shared-buffer-unavailable" : "capture-failed", "N\xE3o foi poss\xEDvel iniciar a captura de v\xEDdeo.");
    }
    return sink.track;
  }
  var screenRecordingDenied = "Permita a Grava\xE7\xE3o de Tela para o Screenshare em Ajustes do Sistema \u203A Privacidade e Seguran\xE7a e abra o app de novo.";
  function noteFrame(wallUs) {
    videoMetrics.received++;
    if (videoMetrics.latencyUs.push(Date.now() * 1e3 - wallUs) > 600) videoMetrics.latencyUs.shift();
  }
  async function videoSink(onFailed) {
    const g = globalThis;
    if (typeof g.VideoFrame === "undefined" || !hasGenerator && !isMac) {
      throw new NativeCaptureError("generator-unavailable", "Este WebView n\xE3o oferece um gerador de v\xEDdeo.");
    }
    if (hasGenerator) {
      const generator = new g.MediaStreamTrackGenerator({ kind: "video" });
      const writer = generator.writable.getWriter();
      return {
        track: generator,
        frame(message) {
          const buffer = sharedSlots.get(message?.slot);
          if (buffer === void 0 || buffer.generation !== message.generation) return;
          try {
            noteFrame(message.wallUs);
            if (writer.desiredSize !== null && writer.desiredSize <= 0) {
              videoMetrics.droppedWriter++;
              return;
            }
            const data = new Uint8Array(buffer.data, sharedHeader, message.width * message.height * 4);
            const frame = new g.VideoFrame(data, { format: "BGRA", codedWidth: message.width, codedHeight: message.height, timestamp: message.timestampUs });
            videoMetrics.written++;
            writer.write(frame).catch(() => {
              frame.close();
              onFailed();
            });
          } finally {
            new Uint8Array(buffer.data)[0] = 0;
          }
        },
        async close() {
          await writer.close().catch(() => {
          });
          generator.stop();
        }
      };
    }
    const worker = new Worker("videoWorker.js");
    const track = await new Promise((resolve, reject) => {
      worker.onmessage = ({ data }) => resolve(data.track);
      worker.onerror = () => reject(new NativeCaptureError("generator-unavailable", "Este WebView n\xE3o oferece um gerador de v\xEDdeo."));
    });
    worker.onmessage = ({ data }) => {
      if (data?.dropped) {
        videoMetrics.droppedWriter++;
        videoMetrics.written--;
      }
    };
    return {
      track,
      frame(message) {
        if (!(message instanceof ArrayBuffer)) return;
        noteFrame(new DataView(message).getFloat64(16, true));
        videoMetrics.written++;
        worker.postMessage(message, [message]);
      },
      async close() {
        track.stop();
        worker.terminate();
      }
    };
  }
  async function stopVideo(track) {
    if (track != null && currentVideo?.track !== track) return;
    await currentVideo?.stop();
  }
  function videoSummary() {
    const sorted = [...videoMetrics.latencyUs].sort((a, b) => a - b);
    const at = (q) => sorted.length ? Math.round(sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))]) : null;
    return { received: videoMetrics.received, written: videoMetrics.written, droppedWriter: videoMetrics.droppedWriter, latencyUsP50: at(0.5), latencyUsP95: at(0.95), latencyUsMax: at(1) };
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
    enumerateSources: () => invoke("enumerate_sources").catch((error) => {
      throw String(error).includes("screen-recording-denied") ? new NativeCaptureError("screen-recording-denied", screenRecordingDenied) : error;
    }),
    captureThumbnail: (hwnd, rect) => invoke("capture_thumbnail", { hwnd, rect }),
    getAudioMetrics: () => ({ ...metrics }),
    startVideoCapture: startVideo,
    stopVideoCapture: stopVideo,
    getVideoMetrics: async () => ({ js: videoSummary(), native: isMac ? null : await invoke("video_capture_metrics") }),
    // Native capture commands (Windows and macOS), not the common shell API.
    isAvailable: navigator.userAgent.includes("Windows") || isMac,
    // A native audio track can be built: a generator (WebView2) or an AudioWorklet (WebKit).
    nativeAudio: hasGenerator && typeof globalThis.AudioData !== "undefined" || isMac && typeof globalThis.AudioWorkletNode !== "undefined",
    mediaAvailable: true,
    autoStartEnabled: () => invoke("plugin:autostart|is_enabled"),
    setAutoStart: (enabled) => invoke(enabled ? "plugin:autostart|enable" : "plugin:autostart|disable"),
    hideMainWindow: () => invoke("hide_main_window"),
    showMainWindow: () => invoke("show_main_window"),
    quitApp: () => invoke("quit_app"),
    setTrayState: (kind, active, participants, detail) => invoke("set_tray_state", { kind, active, participants, detail })
  };
  void listen("tray-action", ({ payload }) => {
    window.dispatchEvent(new CustomEvent("screenshare:tray-action", { detail: payload }));
  });
  void listen("session-media-available", ({ payload }) => {
    globalThis.__screenshareDesktopBridge.mediaAvailable = payload;
    window.dispatchEvent(new CustomEvent("screenshare:session-media-available", { detail: payload }));
  });
  void listen("viewer-mode", ({ payload }) => {
    window.dispatchEvent(new CustomEvent("screenshare:viewer-mode", { detail: payload }));
  });
})();
