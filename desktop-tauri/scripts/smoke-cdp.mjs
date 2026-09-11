const [port = "9223", username = "desktop-smoke", roomId = "desktop-phase2-smoke", mode = "join"] = process.argv.slice(2)

const pages = await fetch(`http://127.0.0.1:${port}/json`).then((response) => response.json())
const page = pages.find(
  (candidate) =>
    candidate.type === "page" &&
    (candidate.url.startsWith("http://tauri.localhost/") || candidate.url.startsWith("https://screen-share.fly.dev/")),
)
if (!page) throw new Error(`No WebView2 page found on diagnostic port ${port}`)

const socket = new WebSocket(page.webSocketDebuggerUrl)
const pending = new Map()
const consoleMessages = []
let nextId = 1

socket.addEventListener("message", (event) => {
  const message = JSON.parse(event.data)
  if (message.id && pending.has(message.id)) {
    const { resolve, reject } = pending.get(message.id)
    pending.delete(message.id)
    if (message.error) reject(new Error(message.error.message))
    else resolve(message.result)
    return
  }
  if (message.method === "Runtime.consoleAPICalled") {
    consoleMessages.push(message.params.args.map((argument) => argument.value ?? argument.description).join(" "))
  }
})

await new Promise((resolve, reject) => {
  socket.addEventListener("open", resolve, { once: true })
  socket.addEventListener("error", reject, { once: true })
})

function command(method, params = {}) {
  const id = nextId++
  socket.send(JSON.stringify({ id, method, params }))
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }))
}

async function evaluate(expression) {
  const response = await command("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  })
  if (response.exceptionDetails) throw new Error(response.exceptionDetails.text)
  return response.result.value
}

await command("Runtime.enable")

const startup = await evaluate(`({
  title: document.title,
  url: location.href,
  isTauri: typeof window.__TAURI_INTERNALS__ !== "undefined",
  hasGetDisplayMedia: typeof navigator.mediaDevices?.getDisplayMedia === "function"
})`)

if (mode !== "status" && mode !== "stop") {
  await evaluate(`(() => {
    if (window.__SCREENSHARE_SMOKE_PEERS__) return;
    window.__SCREENSHARE_SMOKE_PEERS__ = [];
    const NativePeerConnection = window.RTCPeerConnection;
    window.RTCPeerConnection = function (...args) {
      const peer = new NativePeerConnection(...args);
      window.__SCREENSHARE_SMOKE_PEERS__.push(peer);
      return peer;
    };
    window.RTCPeerConnection.prototype = NativePeerConnection.prototype;
  })()`)

  await evaluate(`(() => {
    document.getElementById("username").value = ${JSON.stringify(username)};
    document.getElementById("roomId").value = ${JSON.stringify(roomId)};
    document.getElementById("joinBtn").click();
    return true;
  })()`)

  await new Promise((resolve) => setTimeout(resolve, 4000))
}

if (mode === "share") {
  await evaluate(`(() => {
    document.getElementById("shareScreenBtn").click();
    document.getElementById("confirm-share").click();
    return true;
  })()`)
  await new Promise((resolve) => setTimeout(resolve, 8000))
}

if (mode === "stop") {
  await evaluate(`document.getElementById("stopSharingBtn").click()`)
  await new Promise((resolve) => setTimeout(resolve, 4000))
}

const room = await evaluate(`({
  displayedRoom: document.getElementById("current-room-id")?.textContent?.trim(),
  participantCount: document.getElementById("participant-count")?.textContent?.trim(),
  roomScreenVisible: !document.getElementById("app-screen")?.classList.contains("hidden"),
  joinScreenHidden: document.getElementById("join-screen")?.classList.contains("hidden"),
  videoElementTracks: document.getElementById("screen-video")?.srcObject?.getVideoTracks()?.length ?? 0,
  stopSharingVisible: !document.getElementById("stopSharingBtn")?.classList.contains("hidden"),
  storedDesktopServerUrl: sessionStorage.getItem("screenshare.desktop.serverUrl")
})`)

const peerConnections = await evaluate(`
  (window.__SCREENSHARE_SMOKE_PEERS__ ?? []).map((peer) => ({
    connectionState: peer.connectionState,
    iceConnectionState: peer.iceConnectionState,
    signalingState: peer.signalingState,
    senders: peer.getSenders().length,
    receivers: peer.getReceivers().length,
    receiverTracks: peer.getReceivers().map((receiver) => ({
      kind: receiver.track?.kind,
      readyState: receiver.track?.readyState
    }))
  }))
`)

const relevantConsoleMessages = consoleMessages.filter((message) =>
  /Joining room|UserList|ScreenShareStarted|Connection state|ICE connection state|Adding screen track|StartScreenShare/.test(message),
)

console.log(JSON.stringify({ startup, room, peerConnections, consoleMessages: relevantConsoleMessages }, null, 2))
socket.close()
