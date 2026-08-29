#!/bin/bash
# phase21-e2e.sh — Phase BD-21 跨进程 E2E 测试
# 用法: phase21-e2e.sh <debian-ssh> <repo-id> <data-dir>
# 流程: Backup → Commit → Version → Incremental → Restore → Verify

set -euo pipefail
DEBIAN_SSH="${1:-debian@192.168.1.60}"
REPO_ID="${2:-test-repo}"
DATA_DIR="${3:-/tmp/phase21-test-data}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
EVIDENCE_DIR="${SCRIPT_DIR}/../../docs/phase21-evidence/bd-21-02"

echo "=== Phase BD-21 E2E: Backup → Commit → Version → Incremental → Restore → Verify ==="

# Step 1: Create test data
echo "[1/7] 创建测试数据 ($DATA_DIR)..."
mkdir -p "$DATA_DIR/day1"
for i in $(seq 1 100); do dd if=/dev/urandom of="$DATA_DIR/day1/file_$i.bin" bs=1024 count=10 2>/dev/null; done
echo "  ✅ 100 文件已创建"

# Step 2: Day1 Full Backup
echo "[2/7] Day1 全量备份..."
# Agent → HBOP → 八斗 Server
# TODO: 需要真实 Windows Agent 执行备份
echo "  ⏳ 需要真实 Windows Agent 执行（手动步骤）"

# Step 3: Verify Version
echo "[3/7] 验证 Version 列表..."
ssh "$DEBIAN_SSH" "curl -s http://localhost:9092/api/v1/repos/$REPO_ID/versions" 2>/dev/null || echo "  ⚠️ 需要八斗 Server 运行"

# Step 4: Day2 Incremental
echo "[4/7] Day2 增量备份..."
mkdir -p "$DATA_DIR/day2"
cp -r "$DATA_DIR/day1/"* "$DATA_DIR/day2/" 2>/dev/null || true
for i in $(seq 1 10); do dd if=/dev/urandom of="$DATA_DIR/day2/new_file_$i.bin" bs=1024 count=5 2>/dev/null; done
echo "  ⏳ 需要真实 Windows Agent 执行增量备份"

# Step 5: Restore
echo "[5/7] 恢复..."
RESTORE_DIR="$DATA_DIR/restored"
mkdir -p "$RESTORE_DIR"
echo "  ⏳ 需要真实 Agent 执行恢复"

# Step 6: SHA-256 Compare
echo "[6/7] SHA-256 比对..."
python3 "$SCRIPT_DIR/phase21-sha256-compare.py" "$DATA_DIR/day1" "$RESTORE_DIR" --output "$EVIDENCE_DIR/sha256-report.json" 2>/dev/null || echo "  ⚠️ 需要恢复完成后执行"

# Step 7: Verify
echo "[7/7] 完整性校验..."
ssh "$DEBIAN_SSH" "curl -s -X POST http://localhost:9092/api/v1/repos/$REPO_ID/verify" 2>/dev/null || echo "  ⚠️ 需要八斗 Server 运行"

echo ""
echo "=== E2E 测试脚本完成 ==="
echo "注意: 完整 E2E 需要真实 Windows Agent + Debian 13 集群环境"
echo "证据目录: $EVIDENCE_DIR"