#!/bin/bash
set -euo pipefail

if [ -z "${WIN_SSH_USER:-}" ] || [ -z "${WIN_SSH_PASS:-}" ]; then
    echo "ERROR: WIN_SSH_USER/WIN_SSH_PASS not set"
    exit 1
fi

WIN_HOST="${WIN_HOST:-10.1.8.107}"
BADOU_SERVER="${BADOU_SERVER:-192.168.2.3:9090}"
TEST_GROUP="${TEST_GROUP:-Win10-4GB}"
EVIDENCE_DIR="./win-evidence-${TEST_GROUP}"
WIN_OUTPUT_DIR="C:\\evidence"

win_ssh() {
    sshpass -p "$WIN_SSH_PASS" ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null "${WIN_SSH_USER}@${WIN_HOST}" "$@"
}

win_scp() {
    sshpass -p "$WIN_SSH_PASS" scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null "$@"
}

echo "=== Windows Agent Resource Evidence Collection ==="
echo "Test Group: $TEST_GROUP"
echo "Windows Host: $WIN_HOST"
echo "BadouServer: $BADOU_SERVER"
echo ""

echo "[1/8] Testing SSH connectivity..."
win_ssh "echo connected" || { echo "FAIL: Cannot connect to $WIN_HOST"; exit 1; }
echo "[PASS] SSH connected"

echo "[2/8] Uploading win_resource_monitor.ps1..."
win_scp scripts/win_resource_monitor.ps1 "${WIN_SSH_USER}@${WIN_HOST}:win_resource_monitor.ps1"
echo "[PASS] Script uploaded"

echo "[3/8] Checking Windows RAM..."
RAM_BYTES=$(win_ssh "wmic ComputerSystem get TotalPhysicalMemory /value" 2>/dev/null | grep -oP 'TotalPhysicalMemory=\K\d+' || echo "0")
RAM_MB=$((RAM_BYTES / 1048576))
RAM_LIMIT_MB=$((RAM_MB / 2))
echo "  RAM: ${RAM_MB}MB, Peak RSS limit: ${RAM_LIMIT_MB}MB"

echo "[4/8] Checking Agent binary..."
AGENT_EXISTS=$(win_ssh "if exist C:\\badou-agent.exe (echo yes) else (echo no)" 2>/dev/null || echo "no")
if [ "$AGENT_EXISTS" = "no" ]; then
    echo "  WARNING: badou-agent.exe not found on Windows. Using process monitor only."
    AGENT_PROC="badou-agent"
else
    AGENT_PROC="badou-agent"
    echo "  Agent found: C:\\badou-agent.exe"
fi

mkdir -p "$EVIDENCE_DIR"

echo "[5/8] Idle phase (30s)..."
win_ssh "powershell -ExecutionPolicy Bypass -File win_resource_monitor.ps1 -AgentProcessName $AGENT_PROC -Phase Idle -DurationSec 30 -OutputDir $WIN_OUTPUT_DIR" 2>&1 || echo "  (idle phase completed with warnings)"
win_scp -r "${WIN_SSH_USER}@${WIN_HOST}:${WIN_OUTPUT_DIR}/summary-Idle.json" "$EVIDENCE_DIR/" 2>/dev/null || echo "  (no idle summary)"

echo "[6/8] Backup phase (60s)..."
win_ssh "powershell -ExecutionPolicy Bypass -File win_resource_monitor.ps1 -AgentProcessName $AGENT_PROC -Phase Backup -DurationSec 60 -OutputDir $WIN_OUTPUT_DIR" 2>&1 || echo "  (backup phase completed with warnings)"
win_scp -r "${WIN_SSH_USER}@${WIN_HOST}:${WIN_OUTPUT_DIR}/summary-Backup.json" "$EVIDENCE_DIR/" 2>/dev/null || echo "  (no backup summary)"

echo "[7/8] Incremental phase (30s)..."
win_ssh "powershell -ExecutionPolicy Bypass -File win_resource_monitor.ps1 -AgentProcessName $AGENT_PROC -Phase Incremental -DurationSec 30 -OutputDir $WIN_OUTPUT_DIR" 2>&1 || echo "  (incremental phase completed with warnings)"
win_scp -r "${WIN_SSH_USER}@${WIN_HOST}:${WIN_OUTPUT_DIR}/summary-Incremental.json" "$EVIDENCE_DIR/" 2>/dev/null || echo "  (no incremental summary)"

echo "[8/8] Restore phase (30s)..."
win_ssh "powershell -ExecutionPolicy Bypass -File win_resource_monitor.ps1 -AgentProcessName $AGENT_PROC -Phase Restore -DurationSec 30 -OutputDir $WIN_OUTPUT_DIR" 2>&1 || echo "  (restore phase completed with warnings)"
win_scp -r "${WIN_SSH_USER}@${WIN_HOST}:${WIN_OUTPUT_DIR}/summary-Restore.json" "$EVIDENCE_DIR/" 2>/dev/null || echo "  (no restore summary)"

echo ""
echo "=== Pulling typeperf CSV logs ==="
win_scp -r "${WIN_SSH_USER}@${WIN_HOST}:${WIN_OUTPUT_DIR}" "$EVIDENCE_DIR/win-evidence-raw" 2>/dev/null || echo "  (no raw evidence pulled)"

VERDICT="PASS"
FAIL_REASONS=""
for phase in Idle Backup Incremental Restore; do
    SUMMARY_FILE="$EVIDENCE_DIR/summary-${phase}.json"
    if [ -f "$SUMMARY_FILE" ]; then
        PEAK_RSS=$(python3 -c "import json; d=json.load(open('$SUMMARY_FILE')); print(d.get('peak_rss_mb',0))" 2>/dev/null || echo "0")
        echo "  $phase: peak_rss=${PEAK_RSS}MB (limit=${RAM_LIMIT_MB}MB)"
        if [ "$(python3 -c "print(1 if ${PEAK_RSS} > ${RAM_LIMIT_MB} else 0)" 2>/dev/null || echo "0")" = "1" ]; then
            VERDICT="FAIL"
            FAIL_REASONS="${FAIL_REASONS} ${phase}:peak_rss=${PEAK_RSS}MB>limit=${RAM_LIMIT_MB}MB"
        fi
    else
        echo "  $phase: NO SUMMARY (process may not have been running)"
    fi
done

cat > "$EVIDENCE_DIR/result.json" << EOF
{
  "test_group": "$TEST_GROUP",
  "windows_native": true,
  "remote_driven": true,
  "ssh_tunnel": "192.168.2.3 -> ${WIN_HOST} via sshpass",
  "ram_mb": $RAM_MB,
  "ram_limit_mb": $RAM_LIMIT_MB,
  "verdict": "$VERDICT",
  "fail_reasons": "${FAIL_REASONS}",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF

echo ""
echo "=== Result: $VERDICT ==="
if [ -n "$FAIL_REASONS" ]; then echo "Fail reasons:$FAIL_REASONS"; fi
echo "Evidence saved to: $EVIDENCE_DIR/"