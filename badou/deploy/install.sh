#!/bin/bash
set -euo pipefail

BADOU_VERSION="${1:-latest}"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/badou"
DATA_DIR="/var/lib/badou"
LOG_DIR="/var/log/badou"
SERVICE_FILE="/etc/systemd/system/badou-server.service"

echo "=== BaDou Server Installation ==="
echo "Version: ${BADOU_VERSION}"

# Check root
if [ "$(id -u)" -ne 0 ]; then
    echo "ERROR: Run as root or with sudo"
    exit 1
fi

# Create badou user
if ! id -u badou >/dev/null 2>&1; then
    echo "Creating badou user..."
    useradd --system --no-create-home --shell /usr/sbin/nologin badou
fi

# Create directories
echo "Creating directories..."
mkdir -p "${CONFIG_DIR}" "${DATA_DIR}" "${LOG_DIR}"
chown badou:badou "${DATA_DIR}" "${LOG_DIR}"

# Install binary
BINARY_PATH="${INSTALL_DIR}/badou-server"
if [ -f "./badou-server" ]; then
    echo "Installing binary from local build..."
    cp ./badou-server "${BINARY_PATH}"
elif [ -f "./target/release/badou-server" ]; then
    echo "Installing binary from release build..."
    cp ./target/release/badou-server "${BINARY_PATH}"
else
    echo "ERROR: badou-server binary not found"
    echo "Build with: cargo build --release -p badou-server"
    exit 1
fi
chmod 755 "${BINARY_PATH}"

# Install CLI
CLI_PATH="${INSTALL_DIR}/badou-cli"
if [ -f "./badou-cli" ]; then
    cp ./badou-cli "${CLI_PATH}"
elif [ -f "./target/release/badou-cli" ]; then
    cp ./target/release/badou-cli "${CLI_PATH}"
fi
chmod 755 "${CLI_PATH}" 2>/dev/null || true

# Install default config
if [ ! -f "${CONFIG_DIR}/server.json" ]; then
    echo "Installing default config..."
    cat > "${CONFIG_DIR}/server.json" << 'JSONEOF'
{
  "listen_addr": "0.0.0.0:50051",
  "data_dir": "/var/lib/badou",
  "cluster": {
    "mode": "single",
    "node_id": "node-1"
  },
  "metrics": {
    "addr": "0.0.0.0:9091",
    "path": "/metrics"
  },
  "tls": {
    "cert_path": "",
    "key_path": "",
    "ca_path": ""
  },
  "jwt": {
    "secret": "change-me-in-production",
    "issuer": "badou"
  }
}
JSONEOF
    chmod 640 "${CONFIG_DIR}/server.json"
    chown badou:badou "${CONFIG_DIR}/server.json"
    echo "WARNING: Default config installed. Edit ${CONFIG_DIR}/server.json before starting."
else
    echo "Config already exists, skipping."
fi

# Install systemd service
echo "Installing systemd service..."
cat > "${SERVICE_FILE}" << 'SVCEOF'
[Unit]
Description=BaDou Backup Server (HyperBackup X Native Storage)
After=network-online.target postgresql.service
Wants=network-online.target

[Service]
Type=simple
User=badou
Group=badou
ExecStart=/usr/local/bin/badou-server --config /etc/badou/server.json
WorkingDirectory=/var/lib/badou
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536
LimitNPROC=4096
StandardOutput=journal
StandardError=journal
SyslogIdentifier=badou-server
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/badou /var/log/badou
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
LockPersonality=true
RestrictRealtime=true

[Install]
WantedBy=multi-user.target
SVCEOF

systemctl daemon-reload
systemctl enable badou-server

echo ""
echo "=== Installation Complete ==="
echo "Binary:     ${BINARY_PATH}"
echo "Config:     ${CONFIG_DIR}/server.json"
echo "Data:       ${DATA_DIR}"
echo "Logs:       ${LOG_DIR}"
echo ""
echo "Start with:  sudo systemctl start badou-server"
echo "Status:      sudo systemctl status badou-server"
echo "Logs:        sudo journalctl -u badou-server -f"