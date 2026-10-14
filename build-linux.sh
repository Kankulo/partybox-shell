#!/usr/bin/env bash
# Compila partybox-shell en Linux (release + paquete .deb). Necesita lo que
# instala ./setup-debian.sh (paquetes del sistema, Rust y Tauri CLI).
#
#   ./build-linux.sh
#
# Deja el binario en src-tauri/target/release/partybox-shell y el instalable en
# src-tauri/target/release/bundle/deb/. La primera vez tarda varios minutos.
set -euo pipefail
cd "$(dirname "$0")/src-tauri"

# Si algo se creó antes como root (un build con sudo, un clon como root), esos
# ficheros no son escribibles y cargo/tauri fallan con "permission denied".
if [ "$(id -u)" -ne 0 ]; then
  foreign="$(find .. -not -user "$(id -un)" -print -quit 2>/dev/null)"
  if [ -n "$foreign" ]; then
    echo "Hay ficheros del repo que no son tuyos (p. ej. $foreign)." >&2
    echo "Seguramente se crearon como root. Arréglalo con:" >&2
    echo "  sudo chown -R $(id -un):$(id -gn) \"$(cd .. && pwd)\"" >&2
    exit 1
  fi
fi

# shellcheck disable=SC1091
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
command -v cargo >/dev/null || { echo "No hay Rust: ejecuta ./setup-debian.sh primero." >&2; exit 1; }
cargo tauri --version >/dev/null 2>&1 || { echo "Falta el Tauri CLI: ejecuta ./setup-debian.sh primero." >&2; exit 1; }

# rust-toolchain.toml fija Rust 1.90 solo para este proyecto; rustup lo instala solo.
cargo tauri build --bundles deb

echo
echo "Binario:     $(pwd)/target/release/partybox-shell"
echo "Instalable:  $(ls -1 "$(pwd)"/target/release/bundle/deb/*.deb 2>/dev/null | head -1)"
