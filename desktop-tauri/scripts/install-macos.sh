#!/bin/bash
# Installs Screenshare on macOS without notarization: files fetched by curl carry no
# quarantine attribute, so Gatekeeper does not block the ad hoc signed app.
# Usage: curl -fsSL https://github.com/RafaelrainBR/screenshare-kotlin-js/releases/latest/download/install-macos.sh | bash
set -euo pipefail

URL="${SCREENSHARE_URL:-https://github.com/RafaelrainBR/screenshare-kotlin-js/releases/latest/download/Screenshare-macos-arm64.tar.gz}"

if [ "$(uname -s)" != "Darwin" ] || [ "$(uname -m)" != "arm64" ]; then
  echo "Este instalador é só para Macs com Apple Silicon (M1 ou mais novo)." >&2
  exit 1
fi

DEST=/Applications
[ -w "$DEST" ] || DEST="$HOME/Applications"
mkdir -p "$DEST"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Baixando Screenshare..."
curl -fL --progress-bar "$URL" -o "$TMP/app.tar.gz"
tar -xzf "$TMP/app.tar.gz" -C "$TMP"
[ -d "$TMP/Screenshare.app" ] || { echo "Pacote inválido: Screenshare.app não encontrado." >&2; exit 1; }

osascript -e 'quit app "Screenshare"' >/dev/null 2>&1 || true
rm -rf "$DEST/Screenshare.app"
mv "$TMP/Screenshare.app" "$DEST/"

echo "Instalado em $DEST/Screenshare.app"
echo "Na primeira captura, permita Gravação de Tela em Ajustes do Sistema > Privacidade e Segurança."
echo "Depois de atualizar, se a captura parar, remova e adicione o Screenshare de novo nessa lista."
open "$DEST/Screenshare.app"
