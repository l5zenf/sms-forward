#!/usr/bin/env bash
# Post-deploy setup script for gg-guard on Raspberry Pi.
# Run as root (or via sudo). Adjust paths as needed.
set -euo pipefail

USER_NAME="gg-guard"
GROUP_NAME="gg-guard"
INSTALL_DIR="/usr/local/bin"
ETC_DIR="/etc/gg-guard"
VAR_DIR="/var/lib/gg-guard"

# 1. Disable ModemManager (it steals the AT port)
echo "Disabling ModemManager..."
systemctl disable --now ModemManager 2>/dev/null || true

# 2. Create service user
if ! id "$USER_NAME" &>/dev/null; then
    echo "Creating user $USER_NAME..."
    useradd --system --home "$VAR_DIR" --shell /usr/sbin/nologin "$USER_NAME"
fi

# 3. Create directories
echo "Creating config and data directories..."
mkdir -p "$ETC_DIR" "$VAR_DIR"

# 4. Place files
echo "Installing binary..."
install -m 0755 target/release/gg-guard "$INSTALL_DIR/gg-guard"

echo "Installing config template..."
install -m 0644 config.toml.example "$ETC_DIR/config.toml"
# Don't overwrite existing config silently
if [ ! -f "$ETC_DIR/.env" ]; then
    install -m 0600 .env.example "$ETC_DIR/.env"
fi

echo "Installing systemd unit..."
install -m 0644 gg-guard.service /etc/systemd/system/

# 5. Permissions
echo "Setting ownership..."
chown -R "$USER_NAME:$GROUP_NAME" "$VAR_DIR" "$ETC_DIR"

# 6. Give the service user read access to the serial port
echo "Adding $USER_NAME to dialout group..."
usermod -aG dialout "$USER_NAME"

# 7. Enable & restart
echo "Enabling and starting service..."
systemctl daemon-reload
systemctl enable --now gg-guard

echo
echo "Setup complete. Check service with: systemctl status gg-guard"
echo "Check logs with: journalctl -u gg-guard -f"
