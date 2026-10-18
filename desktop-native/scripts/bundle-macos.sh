#!/bin/bash
# Wraps the release binary in "Screenshare nativo.app" (ad hoc signed) and a tar.gz for install-macos.sh.
# Usage: desktop-native/scripts/bundle-macos.sh <version> <output dir>   (after cargo build --release)
set -euo pipefail

VERSION="$1"
OUT="$2"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/desktop-native/target}"
APP="$OUT/Screenshare nativo.app"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$TARGET/release/desktop-native" "$APP/Contents/MacOS/Screenshare-native"
cp "$ROOT/desktop-tauri/src-tauri/icons/icon.icns" "$APP/Contents/Resources/icon.icns"
# The usage texts are the Tauri app's (desktop-tauri/src-tauri/Info.plist); without them macOS denies
# the camera and microphone. 14.2 is desktop-capture's Swift target.
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleIdentifier</key>
	<string>com.rafaelrain.screenshare.native</string>
	<key>CFBundleName</key>
	<string>Screenshare nativo</string>
	<key>CFBundleDisplayName</key>
	<string>Screenshare nativo</string>
	<key>CFBundleExecutable</key>
	<string>Screenshare-native</string>
	<key>CFBundleIconFile</key>
	<string>icon</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$VERSION</string>
	<key>CFBundleVersion</key>
	<string>$VERSION</string>
	<key>LSMinimumSystemVersion</key>
	<string>14.2</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSAudioCaptureUsageDescription</key>
	<string>Permite compartilhar o áudio do aplicativo escolhido com a sala.</string>
	<key>NSCameraUsageDescription</key>
	<string>Permite compartilhar sua câmera com a sala.</string>
	<key>NSMicrophoneUsageDescription</key>
	<string>Permite transmitir sua voz para a sala.</string>
</dict>
</plist>
PLIST
# Ad hoc, like the Tauri app: enough for the privacy prompts; install-macos.sh avoids the quarantine.
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"
tar -czf "$OUT/Screenshare-native-macos-arm64.tar.gz" -C "$OUT" "Screenshare nativo.app"
sed "s/__VERSION__/$VERSION/" "$ROOT/desktop-native/scripts/install-macos.sh" > "$OUT/install-macos-native.sh"
echo "$APP"
