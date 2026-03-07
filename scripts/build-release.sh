#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_ROOT="${PROJECT_ROOT}/.build/target"
cd "${PROJECT_ROOT}"

echo "Building release binary..."
cargo build --release

BIN_SRC="${TARGET_ROOT}/release/codex-discord-presence"
if [[ ! -f "${BIN_SRC}" ]]; then
  echo "Release binary not found at ${BIN_SRC}" >&2
  exit 1
fi

case "$(uname -s)" in
  Linux*) RELEASE_DIR="${PROJECT_ROOT}/releases/linux" ;;
  Darwin*) RELEASE_DIR="${PROJECT_ROOT}/releases/macos" ;;
  *) RELEASE_DIR="${PROJECT_ROOT}/releases/unknown" ;;
esac

mkdir -p "${RELEASE_DIR}"
BIN_OUT="${RELEASE_DIR}/codex-discord-rich-presence"
cp "${BIN_SRC}" "${BIN_OUT}"
chmod +x "${BIN_OUT}"

ICON_SRC="${PROJECT_ROOT}/assets/branding/codex-app.png"
if [[ -f "${ICON_SRC}" ]]; then
  cp "${ICON_SRC}" "${RELEASE_DIR}/codex-app.png"
fi

echo "Ready:"
echo " - ${BIN_OUT}"
