#!/usr/bin/env bash
# Prepara un Debian 13 (Trixie) o Ubuntu reciente y compila partybox-shell.
# Se ejecuta EN el mini-PC, dentro del repo clonado:
#
#   ./setup-debian.sh              (o ./setup-debian.sh --devtools para compilar con
#                                   el inspector web; ver build-linux.sh)
#
# Instala los paquetes, Rust y el Tauri CLI, y llama a ./build-linux.sh.
# Idempotente: se puede repetir. Para recompilar después de un `git pull`
# basta con ./build-linux.sh.
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"

command -v apt-get >/dev/null || { echo "Este script es solo para Debian/Ubuntu (apt)." >&2; exit 1; }

# Un Debian mínimo puede no traer sudo. Como root no hace falta; si no hay ni
# lo uno ni lo otro, mejor avisar que fallar a media instalación.
if [ "$(id -u)" -eq 0 ]; then
  SUDO=""
  echo "Aviso: ejecutándose como root; es mejor usar tu usuario normal con sudo." >&2
elif command -v sudo >/dev/null; then
  SUDO="sudo"
else
  cat >&2 <<EOF
No hay sudo y no eres root. Instálalo una vez (te pedirá la contraseña de root):

  su -l -c 'apt-get install -y sudo && usermod -aG sudo $USER'

Cierra la sesión, vuelve a entrar y repite ./setup-debian.sh.
EOF
  exit 1
fi

echo "==> Paquetes del sistema"
$SUDO apt-get update
# Compilar: los requisitos de Tauri v2 para Debian (ver README).
# Cámara: WebKitGTK captura vía GStreamer y PipeWire; v4l-utils es para
# comprobar la cámara con v4l2-ctl. La lista de GStreamer/PipeWire es una
# apuesta razonable, no está confirmado qué es lo mínimo.
$SUDO apt-get install -y \
  build-essential curl wget file pkg-config \
  libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-pipewire \
  pipewire pipewire-pulse wireplumber \
  v4l-utils

# Acceso a /dev/video*: surte efecto al volver a iniciar sesión.
if [ "$(id -u)" -ne 0 ] && ! id -nG "$USER" | grep -qw video; then
  echo "==> Añadiendo $USER al grupo video (reinicia la sesión después)"
  $SUDO usermod -aG video "$USER"
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

"$root/build-linux.sh" "$@"

echo
echo "Siguiente paso:  v4l2-ctl --list-devices   y luego   ./run-kiosk.sh --windowed"
