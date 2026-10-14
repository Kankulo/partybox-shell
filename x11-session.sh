#!/usr/bin/env bash
# Sesión X11 mínima para el kiosko: la lanza ./run-kiosk.sh (vía startx) cuando
# no hay escritorio. No se ejecuta a mano. Si el shell termina, la sesión X
# termina con él y se vuelve a la consola.
set -u
here="$(cd "$(dirname "$0")" && pwd)"

# Sin protector de pantalla ni apagado de pantalla: es un kiosko.
xset s off -dpms s noblank 2>/dev/null || true

# El shell necesita un gestor de ventanas para el pantalla completa.
openbox &

exec "$here/run-kiosk.sh" "$@"
