#!/usr/bin/env bash
# Protocolo live opt-in Fase 6A — no forma parte del build ni de CI.
set -euo pipefail

usage() {
  echo "Uso: $0 --scratch DIR [--baseline dgvoodoo|dxvk-only]" >&2
  echo "  DIR debe ser un directorio vacío o de prueba fuera de ~/.local/share/ro-launcher" >&2
  exit 1
}

SCRATCH=""
BASELINE="dgvoodoo"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --scratch)
      SCRATCH="${2:-}"
      shift 2
      ;;
    --baseline)
      BASELINE="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      ;;
    *)
      usage
      ;;
  esac
done

[[ -n "$SCRATCH" ]] || usage

LAUNCHER_DATA="${XDG_DATA_HOME:-$HOME/.local/share}/ro-launcher"
case "$SCRATCH" in
  "$LAUNCHER_DATA"/*)
    echo "error: scratch bajo datos del launcher: $SCRATCH" >&2
    exit 1
    ;;
esac

if [[ -f "$SCRATCH/.ro-launcher-prefix.json" ]] || [[ -f "$SCRATCH/.ro-launcher-dgvoodoo.json" ]]; then
  echo "error: scratch contiene markers del launcher" >&2
  exit 1
fi

mkdir -p "$SCRATCH"

ZIP_CACHE="/tmp/ro-launcher-d7vk-spike/d7vk-v2.2.zip"
PIN_URL="https://github.com/WinterSnowfall/d7vk/releases/download/v2.2/d7vk-v2.2.zip"
PIN_SIZE=3375635
PIN_SHA256="1a9ffe3639ceb5e2fb1ccb25ef388354b4476bb26bc7edf88a8dc59f2774df38"

if [[ ! -f "$ZIP_CACHE" ]]; then
  mkdir -p "$(dirname "$ZIP_CACHE")"
  curl -fsSL -o "$ZIP_CACHE" "$PIN_URL"
fi

actual_size=$(stat -c '%s' "$ZIP_CACHE")
if [[ "$actual_size" != "$PIN_SIZE" ]]; then
  echo "error: tamaño del zip $actual_size != $PIN_SIZE" >&2
  exit 1
fi

actual_sha=$(sha256sum "$ZIP_CACHE" | awk '{print $1}')
if [[ "$actual_sha" != "$PIN_SHA256" ]]; then
  echo "error: sha256 del zip no coincide con el pin" >&2
  exit 1
fi

EXTRACT="$SCRATCH/d7vk-extract"
rm -rf "$EXTRACT"
mkdir -p "$EXTRACT"
unzip -q "$ZIP_CACHE" -d "$EXTRACT"

DLL_SRC="$EXTRACT/d7vk-v2.2/x32/ddraw.dll"
if [[ ! -f "$DLL_SRC" ]]; then
  echo "error: layout inesperado; falta d7vk-v2.2/x32/ddraw.dll" >&2
  exit 1
fi

GAME_DIR="$SCRATCH/game-dir"
mkdir -p "$GAME_DIR"
cp "$DLL_SRC" "$GAME_DIR/ddraw.dll"

cat <<EOF
Spike live preparado (solo filesystem; no lanza Wine ni el launcher).

  scratch:     $SCRATCH
  baseline:    $BASELINE
  ddraw.dll:   $GAME_DIR/ddraw.dll

Siguiente paso manual (registrar en ADR-005 matriz live):
  - Configurar WINEPREFIX/runner de prueba separado del usuario
  - export WINEDLLOVERRIDES=ddraw=n,b (y DXVK/dgVoodoo según baseline)
  - Lanzar cliente RO; capturar DXVK_HUD=1 o D7VK_LOG_PATH
  - Comparar con baseline $BASELINE en el mismo host

No ejecutar este script contra carpetas de juego o prefixes productivos.
EOF
