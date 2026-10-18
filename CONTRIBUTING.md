# Contributing to Screenshare

Bug reports, documentation fixes and testing on Windows, macOS and browsers are welcome. Issues and pull requests can be written in English or Portuguese.

## Report a problem

Check [existing issues](https://github.com/RafaelrainBR/screenshare-kotlin-js/issues) and the [troubleshooting guide](docs/usage.md#troubleshooting), then include:

- Version/tag or commit, OS version, browser/WebView version, and installer or portable app.
- Steps to reproduce, expected behavior and what happened instead.
- Whether each participant used browser or desktop; for audio, which capture source/device was selected.
- A minimal screenshot or error excerpt if it helps.

Remove room codes, private messages, captured content, SDP/ICE details, IP addresses and credentials before sharing diagnostics. Use a disposable demo room when recording a reproduction.

## Send a change

1. Follow the [development guide](docs/development.md); use the [desktop guide](desktop-tauri/README.md) for native work.
2. Keep the change focused and follow the surrounding code's structure.
3. Run the checks relevant to the change and describe the result in the PR. Changes to capture need real-device validation; say explicitly when it was not possible.
4. Update the relevant usage/build docs when behavior changes. Refresh both README entry points and [screenshots](docs/images/README.md) when their content changes.

The browser and desktop share one Kotlin/JS client and protocol. Keep native capture behind the desktop bridge so normal browser builds work independently of Tauri.
