#!/usr/bin/env bash
# Arranca partybox-shell en el mini-PC, en modo kiosko por defecto.
#
#   ./run-kiosk.sh                       kiosko a pantalla completa
#   ./run-kiosk.sh --windowed            en ventana normal (para probar)
#   PARTYBOX_URL=http://otro:8080 ./run-kiosk.sh
#
# Variables (todas opcionales):
#   PARTYBOX_URL     página a mostrar            (por defecto https://partybox-kiosk.vercel.app/)
#                    p. ej. PARTYBOX_URL=http://localhost:5173 para un kiosko local
#   PARTYBOX_ROTATE  0 | 90 | 180 | 270          (por defecto 90, pantalla en vertical)
#   PARTYBOX_CURSOR  hide | show                 (por defecto hide)
#
# Lo que reciba por argumentos se pasa tal cual al shell (ver README). Si no hay
# sesión gráfica (consola de texto), levanta una mínima con X11 + Openbox
# (./x11-session.sh). El shell solo es una ventana que muestra esa URL (la PWA
# del kiosko desplegada en Vercel, así que el mini-PC necesita internet). Si no
# responde, muestra "Conectando…" y entra solo cuando lo haga.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

url="${PARTYBOX_URL:-https://partybox-kiosk.vercel.app/}"
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
  # Sin escritorio (consola de texto): levantar X11 mínimo con ./x11-session.sh,
  # que vuelve a llamar a este script ya con DISPLAY puesto.
  if command -v startx >/dev/null; then
    echo "No hay sesión gráfica: arranco una mínima (X11 + Openbox)..." >&2
    exec startx "$here/x11-session.sh" "$@" -- -nolisten tcp
  fi
  echo "No hay sesión gráfica ni startx. Ejecuta ./setup-debian.sh (instala X11) o abre el escritorio." >&2
  exit 1
fi

# Kiosko salvo que se pida ventana explícitamente.
mode=(--kiosk)
for a in "$@"; do
  [ "$a" = "--windowed" ] && mode=()
done

exec "$bin" --url "$url" --rotate="$rotate" --cursor="$cursor" ${mode[@]+"${mode[@]}"} "$@"
