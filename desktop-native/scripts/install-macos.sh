#!/bin/bash
# Installs the native Screenshare beta on macOS without notarization, like the Tauri app's script:
# files fetched by curl carry no quarantine attribute, so Gatekeeper does not block the ad hoc app.
# Usage: curl -fsSL https://github.com/RafaelrainBR/screenshare-kotlin-js/releases/download/native-v__VERSION__/install-macos-native.sh | bash
set -euo pipefail

URL="${SCREENSHARE_URL:-https://github.com/RafaelrainBR/screenshare-kotlin-js/releases/download/native-v__VERSION__/Screenshare-native-macos-arm64.tar.gz}"
NAME="Screenshare nativo.app"

if [ "$(uname -s)" != "Darwin" ] || [ "$(uname -m)" != "arm64" ]; then
  echo "Este instalador é só para Macs com Apple Silicon (M1 ou mais novo)." >&2
  exit 1
fi

DEST=/Applications
[ -w "$DEST" ] || DEST="$HOME/Applications"
mkdir -p "$DEST"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Baixando o Screenshare nativo..."
curl -fL --progress-bar "$URL" -o "$TMP/app.tar.gz"
tar -xzf "$TMP/app.tar.gz" -C "$TMP"
[ -d "$TMP/$NAME" ] || { echo "Pacote inválido: $NAME não encontrado." >&2; exit 1; }

osascript -e 'quit app "Screenshare nativo"' >/dev/null 2>&1 || true
rm -rf "${DEST:?}/$NAME"
mv "$TMP/$NAME" "$DEST/"

echo "Instalado em $DEST/$NAME"
echo "Na primeira captura, permita Gravação de Tela em Ajustes do Sistema > Privacidade e Segurança."
echo "Depois de atualizar, se a captura parar, remova e adicione o Screenshare nativo de novo nessa lista."
open "$DEST/$NAME"
