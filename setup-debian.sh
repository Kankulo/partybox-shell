#!/usr/bin/env bash
# Prepara un Debian 13 (Trixie) o Ubuntu reciente y compila partybox-shell.
# Se ejecuta EN el mini-PC, dentro del repo clonado:
#
#   ./setup-debian.sh
#
# Instala los paquetes, Rust y el Tauri CLI, y llama a ./build-linux.sh.
# Idempotente: se puede repetir. Para recompilar después de un `git pull`
# basta con ./build-linux.sh.
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"

command -v apt-get >/dev/null || { echo "Este script es solo para Debian/Ubuntu (apt)." >&2; exit 1; }

echo "==> Paquetes del sistema"
sudo apt-get update
# Compilar: los requisitos de Tauri v2 para Debian (ver README).
# Cámara: WebKitGTK captura vía GStreamer y PipeWire; v4l-utils es para
# comprobar la cámara con v4l2-ctl. La lista de GStreamer/PipeWire es una
# apuesta razonable, no está confirmado qué es lo mínimo.
sudo apt-get install -y \
  build-essential curl wget file pkg-config \
  libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-pipewire \
  pipewire pipewire-pulse wireplumber \
  v4l-utils

# Acceso a /dev/video*: surte efecto al volver a iniciar sesión.
if ! id -nG "$USER" | grep -qw video; then
  echo "==> Añadiendo $USER al grupo video (reinicia la sesión después)"
  sudo usermod -aG video "$USER"
fi

echo "==> Rust (rustup)"
if ! command -v rustup >/dev/null && [ ! -x "$HOME/.cargo/bin/rustup" ]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
# shellcheck disable=SC1091
. "$HOME/.cargo/env"

echo "==> Tauri CLI"
if ! cargo tauri --version >/dev/null 2>&1; then
  cargo install tauri-cli --version "^2" --locked
fi

"$root/build-linux.sh"

echo
echo "Siguiente paso:  v4l2-ctl --list-devices   y luego   ./run-kiosk.sh --windowed"
