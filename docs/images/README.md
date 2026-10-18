# Refresh documentation screenshots

The README images are captured from the compiled browser app, not mockups. The demo uses two local peers, a real WebRTC tab capture of `common/Packet.kt`, and sample chat. It does not use a physical microphone/camera or a public room.

## Capture

1. Start the current client/server from the repository root:

   ```bash
   ./gradlew :server-java:run
   ```

2. Make an existing Playwright installation available and install Google Chrome. The script follows the local QA scripts' `PLAYWRIGHT_MODULE_DIR` convention; it does not add a dependency to the app. If needed, install the tool outside the checkout:

   ```bash
   npm install --prefix ../screenshare-doc-tools playwright
   ```

3. Run from the repository root (PowerShell example):

   ```powershell
   $env:PLAYWRIGHT_MODULE_DIR = (Resolve-Path ../screenshare-doc-tools/node_modules).Path
   node scripts/capture-docs.mjs
   ```

   Bash equivalent:

   ```bash
   PLAYWRIGHT_MODULE_DIR="$(cd ../screenshare-doc-tools/node_modules && pwd)" node scripts/capture-docs.mjs
   ```

`SCREENSHARE_DOCS_URL` overrides `http://localhost:8080`; only loopback hosts are accepted. Choose the actual server port if 8080 is occupied. The script opens a temporary loopback HTTP source, selects only that demo tab for capture, waits for decoded remote video and chat delivery, checks the participant count/runtime errors, and closes its browser/source server afterward.

## Review before committing

Inspect all four images at full size: `join_screen.png`, `room_sharing.png`, `sidebar.png` and `share_settings.png`. Check that the shared source is readable, no screen is blank, controls fit, fonts load and no private data is present. Commit the script and images together when behavior changes. The script replaces only these documentation images.

These screenshots demonstrate browser UI and a local media session; they do not validate native capture, installers or performance on remote networks.
