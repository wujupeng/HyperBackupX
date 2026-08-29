#!/bin/bash
set -euo pipefail

NODE_ID="${1:?Usage: cluster-join.sh <node-id> <node-addr> <node-port> <leader-addr> <leader-port>}"
NODE_ADDR="${2:?Missing node-addr}"
NODE_PORT="${3:?Missing node-port}"
LEADER_ADDR="${4:?Missing leader-addr}"
LEADER_PORT="${5:?Missing leader-port}"
CONFIG_FILE="/etc/badou/server.json"

echo "=== BaDou Cluster Join ==="
echo "New Node:  ${NODE_ID} @ ${NODE_ADDR}:${NODE_PORT}"
echo "Leader:    ${LEADER_ADDR}:${LEADER_PORT}"

if [ "$(id -u)" -ne 0 ]; then
    echo "ERROR: Run as root or with sudo"
    exit 1
fi

if [ ! -f "${CONFIG_FILE}" ]; then
    echo "ERROR: Config file ${CONFIG_FILE} not found. Run install.sh first."
    exit 1
fi

# Update config for cluster join
cat > "${CONFIG_FILE}" << JSONEOF
{
  "listen_addr": "0.0.0.0:${NODE_PORT}",
  "data_dir": "/var/lib/badou",
  "cluster": {
    "mode": "raft",
    "node_id": "${NODE_ID}",
    "node_addr": "${NODE_ADDR}",
    "node_port": ${NODE_PORT},
    "cluster_name": "badou-cluster",
    "join_to": "${LEADER_ADDR}:${LEADER_PORT}"
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
    echo "=== Node ${NODE_ID} joined cluster successfully ==="
    echo "Check cluster status:"
    echo "  sudo badou-cli cluster status"
else
    echo "ERROR: badou-server failed to start"
    journalctl -u badou-server --no-pager -n 20
    exit 1
fi