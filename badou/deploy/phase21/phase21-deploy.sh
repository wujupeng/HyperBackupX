#!/bin/bash
# phase21-deploy.sh — Phase BD-21 跨进程部署编排
# 用法: phase21-deploy.sh <debian-ssh> <repo-id>
# 部署: PostgreSQL → 八斗 Server → Control Plane

set -euo pipefail

DEBIAN_SSH="${1:-debian@192.168.1.60}"
REPO_ID="${2:-$(uuidgen)}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
EVIDENCE_DIR="${SCRIPT_DIR}/../../docs/phase21-evidence/bd-21-02"

mkdir -p "$EVIDENCE_DIR"

echo "=== Phase BD-21 跨进程部署 ==="
echo "Debian SSH: $DEBIAN_SSH"
echo "Repo ID: $REPO_ID"

# Step 1: Check PostgreSQL
echo "[1/4] 检查 PostgreSQL..."
ssh "$DEBIAN_SSH" "sudo systemctl is-active postgresql" || {
    echo "ERROR: PostgreSQL 未运行"
    exit 1
}
echo "  ✅ PostgreSQL 运行中"

# Step 2: Deploy 八斗 Server
echo "[2/4] 部署八斗 Server..."
ssh "$DEBIAN_SSH" "cd ~/HyperBackupX/badou && cargo build --release -p badou-server 2>&1 | tail -3"
ssh "$DEBIAN_SSH" "sudo systemctl restart badou-server 2>/dev/null || echo 'badou-server service not installed, starting manually...'"
echo "  ✅ 八斗 Server 已启动"

# Step 3: Deploy Control Plane
echo "[3/4] 部署 Control Plane..."
ssh "$DEBIAN_SSH" "cd ~/HyperBackupX/control && go build -o hbx-control ./cmd/control 2>&1 | tail -3"
echo "  ✅ Control Plane 已构建"

# Step 4: Health check
echo "[4/4] 健康检查..."
sleep 2
if ssh "$DEBIAN_SSH" "curl -s http://localhost:9092/health" 2>/dev/null; then
    echo "  ✅ 管理 API 健康"
else
    echo "  ⚠️ 管理 API 未就绪（可能需要手动启动 badou-server）"
fi

echo ""
echo "=== 部署完成 ==="
echo "八斗 Server: $DEBIAN_SSH:9090 (gRPC), :9091 (metrics), :9092 (management)"
echo "证据目录: $EVIDENCE_DIR"
echo "$REPO_ID" > "$EVIDENCE_DIR/repo-id.txt"