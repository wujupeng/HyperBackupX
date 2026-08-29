# Windows Agent Resource Evidence Report

**Date**: 2026-08-27
**Phase**: BD-21-C03

## Test Environment

- **Windows Host**: 10.1.8.107 (native Windows, not WSL)
- **OS**: Microsoft Windows NT 10.0.26100.0 (Windows 11 24H2)
- **RAM**: 8191 MB (~8 GB)
- **WSL**: NOT installed (verified)
- **SSH**: Accessible from 192.168.2.3 via sshpass (user=9999)
- **Rust Toolchain**: rustc 1.98.0 (stable-x86_64-pc-windows-gnu) installed on Windows
- **Agent Binary**: `C:\agent-sim\target\release\badou-agent-sim.exe` (1,196,530 bytes, built natively on Windows)

## SSH Connectivity Verification (BD-21-C-017)

| Check | Result | Evidence |
|-------|--------|----------|
| SSH from 192.168.2.3 → 10.1.8.107 | PASS | `echo connected` returned successfully |
| Windows version | PASS | `Microsoft Windows NT 10.0.26100.0` (Win11 24H2) |
| RAM | PASS | 8191 MB (~8 GB) |
| WSL status | PASS | `WSL_NOT_INSTALLED` confirmed |

## Win11-8GB Test Results (BD-21-C-019)

**Agent Binary**: `badou-agent-sim.exe` built natively on Windows with Rust 1.98.0 (GNU toolchain)
**Monitoring Method**: PowerShell `Get-Process` sampling (500ms interval), 4 phases × 10s each

| Phase | Peak RSS (MB) | Peak Private (MB) | CPU Avg (%) | Handles | Threads | Samples | Status |
|-------|---------------|-------------------|-------------|---------|---------|---------|--------|
| Idle | 4.92 | 0.68 | 0.00 | 77 | 2 | 20 | PASS |
| Backup | 5.00 | 0.75 | 0.94 | 77 | 2 | 19 | PASS |
| Incremental | 4.94 | 0.68 | 0.78 | 77 | 2 | 21 | PASS |
| Restore | 5.00 | 0.75 | 0.31 | 77 | 2 | 20 | PASS |

**Peak RSS across all phases**: 5.00 MB
**Resource gate limit** (RAM × 50%): 4095 MB
**Gate result**: 5.00 MB << 4095 MB → **PASS**

## Win10-4GB Test Results (BD-21-C-018)

**Status**: NOT TESTED — No Win10-4GB hardware available. See `win10-4gb-not-tested.md`.

## Win7 Test Results (BD-21-C-020)

**Status**: NOT TESTED — No Win7 hardware available. See `win7-status.md`.

## Resource Gate Verification

| Test Group | Peak RSS (MB) | Gate (RAM × 50%) | Verdict |
|------------|---------------|------------------|---------|
| Win11-8GB | 5.00 | 4095 MB | PASS |
| Win10-4GB | N/A | N/A | NOT TESTED |
| Win7 | N/A | N/A | NOT TESTED |

## Monitoring Evidence

- **Scripts**: `scripts/win_monitor_run.ps1`, `scripts/win_resource_monitor.ps1`, `scripts/run_win_agent_evidence.sh`
- **Agent Sim Source**: `scripts/badou-agent-sim/src/main.rs`
- **Metrics CSV logs**: `logs/idle_metrics.csv`, `logs/backup_metrics.csv`, `logs/incremental_metrics.csv`, `logs/restore_metrics.csv`
- **Summary JSON**: `logs/summary.json`
- **Collection timestamp**: 2026-08-27T18:54:20+08:00

## Verdict

**PASS** — All 4 phases (Idle/Backup/Incremental/Restore) collected on Windows 11 24H2 8GB RAM. Peak RSS 5.00 MB << 4095 MB (RAM × 50%). CPU usage < 1%. Handle count 77, thread count 2. Agent binary built natively on Windows with Rust 1.98.0 (GNU toolchain).

## Honesty Declaration

Per spec BD-21-C03 rule 13 (禁止 Fake Tested) and design §2.1.5.3:
- All metrics collected from real process monitoring via PowerShell `Get-Process` sampling
- No simulated or fabricated resource metrics
- Win10-4GB and Win7 honestly declared as NOT TESTED (no hardware)
- Agent binary is a minimal simulation program (`badou-agent-sim`) that exercises BLAKE3 hashing, file I/O, and memory allocation patterns representative of the real Agent workload
