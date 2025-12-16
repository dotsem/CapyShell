#!/bin/bash
# CapyShell Setup Script
# Installs the systemd user service for auto-restart on hotplug

set -e

SERVICE_NAME="capyshell.service"
SERVICE_DIR="$HOME/.config/systemd/user"
SERVICE_PATH="$SERVICE_DIR/$SERVICE_NAME"

# Get the binary path (either from cargo or installed location)
if [ -f "./target/release/CapyShell" ]; then
    BINARY_PATH="$(pwd)/target/release/CapyShell"
elif [ -f "./target/debug/CapyShell" ]; then
    BINARY_PATH="$(pwd)/target/debug/CapyShell"
else
    echo "Error: CapyShell binary not found. Run 'cargo build --release' first."
    exit 1
fi

echo "Setting up CapyShell systemd service..."
echo "Binary: $BINARY_PATH"

# Create systemd user directory if needed
mkdir -p "$SERVICE_DIR"

# Check if service already exists
if [ -f "$SERVICE_PATH" ]; then
    echo "Service already exists at $SERVICE_PATH"
    read -p "Overwrite? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 0
    fi
fi

# Write the service file
cat > "$SERVICE_PATH" << EOF
[Unit]
Description=CapyShell Taskbar
After=graphical-session.target

[Service]
Type=simple
ExecStart=$BINARY_PATH
Restart=always
RestartSec=0.2
Environment=DISPLAY=:0
Environment=WAYLAND_DISPLAY=wayland-1

[Install]
WantedBy=default.target
EOF

echo "Created: $SERVICE_PATH"

# Reload systemd
systemctl --user daemon-reload

# Enable and start the service
systemctl --user enable "$SERVICE_NAME"
systemctl --user start "$SERVICE_NAME"

echo ""
echo "✓ CapyShell service installed and started!"
echo ""
echo "Useful commands:"
echo "  systemctl --user status capyshell   # Check status"
echo "  systemctl --user restart capyshell  # Restart"
echo "  systemctl --user stop capyshell     # Stop"
echo "  journalctl --user -u capyshell -f   # View logs"
