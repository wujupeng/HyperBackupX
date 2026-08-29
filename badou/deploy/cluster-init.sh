#!/bin/bash
set -euo pipefail

NODE_ID="${1:-node-1}"
NODE_ADDR="${2:-127.0.0.1}"
NODE_PORT="${3:-50051}"
CLUSTER_NAME="${4:-badou-cluster}"
CONFIG_FILE="/etc/badou/server.json"

echo "=== BaDou Cluster Initialization ==="
echo "Node ID:    ${NODE_ID}"
echo "Node Addr:  ${NODE_ADDR}:${NODE_PORT}"
echo "Cluster:    ${CLUSTER_NAME}"

if [ "$(id -u)" -ne 0 ]; then
    echo "ERROR: Run as root or with sudo"
    exit 1
fi

if [ ! -f "${CONFIG_FILE}" ]; then
    echo "ERROR: Config file ${CONFIG_FILE} not found. Run install.sh first."
    exit 1
fi

# Update config for cluster mode
cat > "${CONFIG_FILE}" << JSONEOF
{
  "listen_addr": "0.0.0.0:${NODE_PORT}",
  "data_dir": "/var/lib/badou",
  "cluster": {
    "mode": "raft",
    "node_id": "${NODE_ID}",
    "node_addr": "${NODE_ADDR}",
    "node_port": ${NODE_PORT},
    "cluster_name": "${CLUSTER_NAME}",
    "initial_peers": [
      "${NODE_ADDR}:${NODE_PORT}"
    ]
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

chown badou:badou "${CONFIG_FILE}"
chmod 640 "${CONFIG_FILE}"

echo "Restarting badou-server..."
systemctl restart badou-server
sleep 2

if systemctl is-active --quiet badou-server; then
    echo "=== Cluster initialized successfully ==="
    echo "Node ${NODE_ID} is the initial leader."
    echo ""
    echo "To add more nodes, run cluster-join.sh on the new node:"
    echo "  sudo ./cluster-join.sh <new-node-id> <new-node-addr> <new-node-port> ${NODE_ADDR} ${NODE_PORT}"
else
    echo "ERROR: badou-server failed to start"
    journalctl -u badou-server --no-pager -n 20
    exit 1
fi