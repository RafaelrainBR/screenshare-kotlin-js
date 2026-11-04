# Use Screenshare

[Overview](../README.md) · [Documentation](README.md)

## Choose how to connect

**Browser:** open [screen-share.fly.dev](https://screen-share.fly.dev/). Use a current browser with WebRTC and screen-capture support. Screen sharing and audio options vary by browser and operating system; mobile browsers may join without supporting screen capture.

**Desktop:** open the [download page](https://screen-share.fly.dev/about/download-en.html) or the [latest release](https://github.com/RafaelrainBR/screenshare/releases/latest) and choose the package for your machine. The desktop app is a native client (Rust and iced, no WebView). It connects to the public Screenshare server by default and joins the same rooms as browser users.

| Platform | Download | Notes |
|---|---|---|
| Windows x64 | `Screenshare-native_<version>_x64-setup.exe` | Per-user installer, no administrator needed |
| Windows x64 | `Screenshare-native_<version>_x64-portable.exe` | Runs without installing |
| macOS Apple Silicon | `install-macos-native.sh` and `Screenshare-native-macos-arm64.tar.gz` | Self-signed app; not notarized |

All three update themselves from the tray menu. `SHA256SUMS` lists download checksums. Windows builds are unsigned, so Windows may show an unknown-publisher warning. Use packages from this repository's releases. There are no Linux or Intel Mac builds.

### macOS installation

The install script downloads the Apple Silicon archive of its release, installs it into `/Applications` (or `~/Applications` if needed), and opens Screenshare. Download and inspect it before running:

```bash
curl -fL https://github.com/RafaelrainBR/screenshare/releases/latest/download/install-macos-native.sh -o install-macos-native.sh
less install-macos-native.sh
bash install-macos-native.sh
```

The app is not notarized; a browser-downloaded archive may trigger Gatekeeper. The provided script downloads the archive with `curl`. Allow screen recording, microphone and camera when prompted. Application audio uses Core Audio taps and requires macOS 14.2 or later.

## Create or join a room

1. Enter a display name in **Seu nome**.
2. Leave **Código da sala** empty to create a room, or paste an existing code. **Criar sala privada** fills in a long random code instead.
3. Click **Entrar na sala**.
4. Click the room code in the conversation header to copy the room link (`https://screen-share.fly.dev/?roomId=…`) for other participants.

The link opens the room in the browser. On Windows, the page also shows **Tem o app? Abrir no app**, which opens the room in the desktop app through its `screenshare:` link. The join screen lists recent rooms; mark one as a favorite to keep it at the top.

Rooms do not require accounts or passwords. Anyone who knows a room code can join; a private room's 128-bit random code cannot be guessed, so only people who receive the link get in. Messages and room membership are held in server memory; chat is not a durable archive and is lost when a room is removed or the server restarts.

## Share a screen and audio

Click the screen button. In a browser, choose a screen, window or tab in its picker. If the picker offers audio, explicitly enable it there. Support for tab audio does not imply support for capturing every application's audio.

In the desktop app, choose a screen or window from the native thumbnails and whether to share its audio. A screen can send computer audio; a window can send the selected application's audio. Stop sharing with the screen stop button; camera and microphone have separate controls.

On Windows, computer-audio capture leaves out call and chat apps (Discord, Teams, Zoom, Skype, Slack, TeamSpeak, Mumble, WhatsApp, Telegram, Signal, Element, Guilded), Screenshare itself (the call and the other screens) and system sounds. Apps opened during the share are picked up within a second, so there is no need to share again. A browser opened from one of those apps still plays, and calls inside a browser are not filtered. If the system does not support per-app audio capture, the audio stops instead of falling back to an unfiltered mix.

### Transmission settings

Open the arrow beside the screen button to choose:

| Preset | Starting point | Use for |
|---|---|---|
| **Jogo** | 60 fps; prioritize smoothness | Games and fast motion |
| **Vídeo** (default) | 30 fps; prioritize sharpness | Video and general sharing |
| **Código e texto** | 10 fps; prioritize sharpness | Text, code and slides |

Customize **Quadros/s**, **Se piorar** (quality preference when the network tightens), and **Bitrate**. Native desktop video keeps source dimensions. Actual quality depends on capture support, encoding, bandwidth and the receiving device.

Click a stream to focus it; use the preview rail to switch streams or return to the grid. **Janela flutuante** pops a stream out into a small window. Each viewer can hide video or adjust another participant's volume locally. When your share reaches its limit, the warning offers one click to lighten it.

## Voice, camera and conversation

Microphone and camera start off. Use their buttons to enable them and the adjacent arrows to select devices. In the browser, the microphone menu also has **Reduzir ruído do microfone**. The speaker button mutes local output; its menu selects the output device (browser support varies) and turns off the join/leave/share tones.

Type a message and press Enter, or use the send button on narrow screens. **Na sala**, above the messages, lists who is in the room. Hide/show the conversation with its arrow. The leave button lets you leave or switch rooms.

On desktop, closing the main window keeps the tray available. Use the tray menu to open the room, manage media, update or quit. The optional **Iniciar com o sistema e voltar a esta sala** setting controls startup at login and room rejoin.

**Algo deu errado?** (in the room on the web, in the tray on desktop) sends a short note with diagnostics to the maintainer, without your name or room.

## Updates

The desktop app checks GitHub Releases for a newer version. When one exists, the tray menu and the join screen show **Atualizar para a versão X.Y.Z**: the app downloads its package, checks it against `SHA256SUMS`, replaces itself and reopens. The installer, the portable EXE and the macOS app all update this way. If a release has no package for your system, the item opens the release page instead.

Every macOS release is signed with the same certificate, so the Screen Recording permission survives updates. If capture stops working anyway, remove Screenshare from **System Settings → Privacy & Security → Screen Recording**, then add it again and restart.

## Usage statistics

The hosted site and the released desktop app send error reports and anonymous usage statistics to Sentry: how many rooms and shares happen, how long they last and why quality drops, in ranges. They never include your name, the room code, your IP address or what you share. Turn the statistics off with **Enviar estatísticas de uso anônimas** in the speaker menu; error reports keep going. Self-hosted instances and local builds send nothing.

## Troubleshooting

| Symptom | Check |
|---|---|
| Screen sharing is unavailable | Use HTTPS or `localhost`; check browser support and OS screen-recording permissions. |
| Microphone or camera is denied | Allow browser and OS access; select an available device. Desktop permission recovery can open OS settings. |
| Shared video works but audio is silent | Check source selection and local/remote volume. In a browser, verify the picker's audio checkbox and source support. |
| A peer never connects | Test another network. Restricted networks may need a working TURN relay; see [self-hosting](self-hosting.md#stun-and-turn). |
| macOS capture stopped after an update | Remove and re-add Screen Recording permission, then restart. |
| Protected content is black or silent | DRM, HDCP, minimized-window and platform restrictions can prevent capture. |
| Messages disappear after reconnect/restart | Room chat is in-memory, not persistent storage. |

For bug reports, include app version, platform, steps, expected/actual behavior and whether each peer uses browser or desktop. See [contributing](../CONTRIBUTING.md) before attaching logs or screenshots.
