# Android app

[Documentation](../docs/README.md)

The Android Screenshare: Jetpack Compose on top of the desktop's Rust engine ([`desktop-native`](../desktop-native)), called through JNI ([`engine/`](engine)). It joins the same rooms as the browser and the desktop, with the same protocol, negotiation and quality rules.

- `app/`: the app. `RoomService` owns the engine while you are in a room (a foreground service with a notification); `ScreenCapture` feeds MediaProjection frames and the phone's audio to it; `VideoTile` draws the engine's decoded frames with libwebrtc's GL renderer.
- `engine/`: a `cdylib` of JNI exports (`Engine.kt`). The engine takes JSON requests and hands back the room state as JSON.

## Build and test

Needs the Android SDK (platform 37, build-tools 36), NDK 28.2.13676358, Rust from `engine/rust-toolchain.toml` and [`cargo-ndk`](https://github.com/bbqsrc/cargo-ndk). The Gradle build runs `cargo ndk` before packaging.

```bash
./gradlew :app:testDebugUnitTest :app:assembleDebug
```

- `-Pabis=x86_64` builds only the emulator's ABI (debug default: arm64-v8a, armeabi-v7a and x86_64).
- On Windows, point `CARGO_TARGET_DIR` at a short path such as `C:/cta`: libwebrtc's include paths overflow `MAX_PATH`.
- Local server: `./gradlew :server-java:run` in the repository root, then `adb reverse tcp:8080 tcp:8080` and build with `-Pserver=ws://localhost:8080/` (the engine allows `ws://` only on localhost).

Local and pull request builds have no Sentry DSN and send nothing.

## Release

```bash
git tag -a android-vX.Y.Z-rc.N -F notes.txt
git push origin android-vX.Y.Z-rc.N
```

The tag only dispatches [`release-android.yml`](../.github/workflows/release-android.yml) on `master`, which signs the APK, uploads the R8 mapping and native symbols to Sentry and publishes the release **with `--latest=false`**: "latest" belongs to the desktop (installed Tauri apps read it). `notes.txt` is a title line, a blank line and pt-BR bullets of what changed.

`versionCode` is X·1000000 + Y·10000 + Z·100 + N, and 99 for the final release, so each release candidate installs over the previous one and the final over all of them. The in-app updater offers only final releases.

The signing key lives in the repository secrets (`ANDROID_KEYSTORE_BASE64` and the passwords) and in an offline copy with the maintainer. Without it, installed apps can never be updated again.
