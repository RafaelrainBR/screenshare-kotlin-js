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
    getAudioMetrics: () => ({ ...metrics }),
    // This capability covers Windows capture commands, not the common shell API.
    isAvailable: navigator.userAgent.includes("Windows"),
    mediaAvailable: true,
    autoStartEnabled: () => invoke("plugin:autostart|is_enabled"),
    setAutoStart: (enabled) => invoke(enabled ? "plugin:autostart|enable" : "plugin:autostart|disable"),
    hideMainWindow: () => invoke("hide_main_window"),
    showMainWindow: () => invoke("show_main_window"),
    quitApp: () => invoke("quit_app"),
    setTrayState: (kind, active, participants) => invoke("set_tray_state", { kind, active, participants })
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
