#!/usr/bin/env bash
# Builds aegis-agent in release mode and installs it as a systemd service.
# Not run automatically by anything — review it, then run it yourself:
#   ./systemd/install.sh
# Requires sudo; you'll be prompted interactively.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NAME=aegis-agent
UNIT_NAME=aegis-agent.service

echo "==> Building release binary"
cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" --bin "$BIN_NAME"

echo "==> Installing binary to /usr/local/bin (sudo)"
sudo install -m 755 "$REPO_ROOT/target/release/$BIN_NAME" "/usr/local/bin/$BIN_NAME"

echo "==> Installing unit file to /etc/systemd/system (sudo)"
sudo install -m 644 "$REPO_ROOT/systemd/$UNIT_NAME" "/etc/systemd/system/$UNIT_NAME"

echo "==> Reloading systemd and enabling the service"
sudo systemctl daemon-reload
sudo systemctl enable --now "$UNIT_NAME"

echo "==> Done. Check status with:"
echo "    systemctl status $UNIT_NAME"
echo "    journalctl -u $UNIT_NAME -f"
