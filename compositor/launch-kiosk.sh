#!/usr/bin/env bash
# Phase 3, working path: launch the Aegis frontend as a single-purpose kiosk
# under cage. cage is a minimal Wayland compositor that runs exactly one
# application fullscreen with no window management — precisely the model Aegis
# wants for a console-like appliance. This is what ships until the custom
# Smithay compositor (same directory, aegis-compositor) reaches parity.
#
#   ./launch-kiosk.sh                 # runs the built Electron app under cage
#   AEGIS_KIOSK=1 is exported so the frontend goes frameless/fullscreen.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTEND="$REPO_ROOT/frontend"

if ! command -v cage >/dev/null; then
  echo "cage not installed (apt-get install cage)" >&2
  exit 1
fi

export AEGIS_KIOSK=1

# cage runs one app and exits when it exits — exactly the kiosk contract.
# --  everything after is the application + args cage launches fullscreen.
exec cage -- "$FRONTEND/node_modules/.bin/electron" "$FRONTEND"
