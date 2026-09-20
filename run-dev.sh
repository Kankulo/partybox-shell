#!/usr/bin/env bash
# Runs the shell in development (debug build, with the Kiosco menu and shortcut).
#
#   ./run-dev.sh                      shows http://localhost:5173 (the PartyBox kiosk app)
#   ./run-dev.sh http://otro:8080     shows another URL
#   ./run-dev.sh --windowed           extra flags go straight to the app
#
# The site must be running (e.g. ./dev.sh in the parent folder); if it isn't
# yet, the shell waits on its "Conectando…" screen and goes in once it is.
set -e
cd "$(dirname "$0")/src-tauri"

args=()
for a in "$@"; do
  case "$a" in
    --*) args+=("$a") ;;
    *)   args+=(--url "$a") ;;
  esac
done

exec cargo tauri dev -- -- "${args[@]}"
