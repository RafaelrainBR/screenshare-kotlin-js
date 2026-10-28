# Desktop app

[Documentation](../docs/README.md) · [Plan and history](../docs/native-desktop-plan.md)

The desktop Screenshare: Rust and [iced](https://iced.rs), no WebView. It speaks the same protocol as the browser client and shares capture code with [`desktop-capture/`](../desktop-capture).

## Build and test

Rust comes from `rust-toolchain.toml`. Run cargo inside this folder: `.cargo/config.toml` (static CRT, to match the prebuilt libwebrtc) is only read from here.

```bash
cargo test --release
cargo run --release
```

On Windows:

- libwebrtc's include paths overflow `MAX_PATH` under a deep checkout; point `CARGO_TARGET_DIR` at a short path such as `C:/t`.
- NVENC needs NVIDIA's `cuda.h` and `cuda.lib` through `CUDA_PATH` (see the CUDA step in `release-native.yml`). Without it the app builds and encodes in software, or through Media Foundation on Intel and AMD GPUs.

A local build has no Sentry DSN and no release tag: it sends nothing and never updates itself.

## Release

```bash
git tag -a native-vX.Y.Z -m "Screenshare X.Y.Z"
git push origin native-vX.Y.Z
```

The tag's run only dispatches [`release-native.yml`](../.github/workflows/release-native.yml) on `master`; that second run builds Windows and macOS and publishes the release as "latest". A suffix (`native-vX.Y.Z-rc.1`) publishes a pre-release, which the updater never offers. The app updates itself from these releases (`src/updater.rs`), so the asset names must not change.
