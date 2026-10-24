#!/bin/bash
# Imports the self-signed signing certificate (secrets MACOS_SIGN_P12, base64, and MACOS_SIGN_PASSWORD)
# into a temporary keychain and exports MACOS_SIGN_IDENTITY for bundle-macos.sh.
# Why: macOS ties Screen Recording and the other permissions to the signature. An ad hoc signature
# changes with every build, so each update would ask again; one certificate kept across releases
# does not. It is free: no Apple Developer account, and without quarantine Gatekeeper does not check it.
# Without the secrets (forks) it does nothing and the bundle stays ad hoc.
set -euo pipefail

if [ -z "${MACOS_SIGN_P12:-}" ]; then
  echo "No MACOS_SIGN_P12: the bundle is signed ad hoc."
  exit 0
fi

KEYCHAIN="$RUNNER_TEMP/sign.keychain-db"
security create-keychain -p "" "$KEYCHAIN"
security set-keychain-settings "$KEYCHAIN"
security unlock-keychain -p "" "$KEYCHAIN"
echo "$MACOS_SIGN_P12" | base64 --decode > "$RUNNER_TEMP/sign.p12"
security import "$RUNNER_TEMP/sign.p12" -k "$KEYCHAIN" -P "$MACOS_SIGN_PASSWORD" -T /usr/bin/codesign
rm -f "$RUNNER_TEMP/sign.p12"
security set-key-partition-list -S apple-tool:,apple: -s -k "" "$KEYCHAIN" > /dev/null
security list-keychains -d user -s "$KEYCHAIN" $(security list-keychains -d user | tr -d '"')
# codesign only signs with a trusted identity; trust this one for code signing on the runner.
security find-certificate -c "Screenshare nativo" -p "$KEYCHAIN" > "$RUNNER_TEMP/sign.pem"
sudo security add-trusted-cert -d -r trustRoot -p codeSign -k /Library/Keychains/System.keychain "$RUNNER_TEMP/sign.pem"
security find-identity -v -p codesigning "$KEYCHAIN"
echo "MACOS_SIGN_IDENTITY=Screenshare nativo" >> "$GITHUB_ENV"
