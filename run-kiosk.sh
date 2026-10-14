#!/usr/bin/env bash
# Arranca partybox-shell en el mini-PC, en modo kiosko por defecto.
#
#   ./run-kiosk.sh                       kiosko a pantalla completa
#   ./run-kiosk.sh --windowed            en ventana normal (para probar)
#   PARTYBOX_URL=http://otro:8080 ./run-kiosk.sh
#
# Variables (todas opcionales):
#   PARTYBOX_URL     página a mostrar            (por defecto http://localhost:5173)
#   PARTYBOX_ROTATE  0 | 90 | 180 | 270          (por defecto 90, pantalla en vertical)
#   PARTYBOX_CURSOR  hide | show                 (por defecto hide)
#
# Lo que reciba por argumentos se pasa tal cual al shell (ver README). Necesita
# una sesión gráfica abierta: el shell solo es una ventana; el kiosko (la PWA)
# tiene que estar sirviéndose en esa URL. Mientras no responda, el shell
# muestra "Conectando…" y entra solo cuando lo haga.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

url="${PARTYBOX_URL:-http://localhost:5173}"
rotate="${PARTYBOX_ROTATE:-90}"
cursor="${PARTYBOX_CURSOR:-hide}"

if [ -x "$here/src-tauri/target/release/partybox-shell" ]; then
  bin="$here/src-tauri/target/release/partybox-shell"
elif command -v partybox-shell >/dev/null; then
  bin="$(command -v partybox-shell)"   # instalado desde el .deb
else
  echo "No hay binario compilado. Ejecuta ./setup-debian.sh (o ./build-linux.sh)." >&2
  exit 1
fi

if [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  echo "No hay sesión gráfica (DISPLAY vacío). Ejecútalo desde el escritorio del mini-PC." >&2
  exit 1
fi

# Kiosko salvo que se pida ventana explícitamente.
mode=(--kiosk)
for a in "$@"; do
  [ "$a" = "--windowed" ] && mode=()
done

exec "$bin" --url "$url" --rotate="$rotate" --cursor="$cursor" ${mode[@]+"${mode[@]}"} "$@"
