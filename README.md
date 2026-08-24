# HyperBackup X

> Next-Generation Encrypted Backup & Disaster Recovery Platform

跨平台、加密、去重、增量、版本化的备份与灾难恢复平台。

## 架构

```
┌─────────────────────────────────────────────────────────┐
│                    HyperBackup X                         │
├──────────┬──────────┬──────────┬───────────────────────┤
│  Agent   │ Control   │   Web    │       Tray            │
│ (Rust)   │  (Go)     │ (React)  │      (Qt6)            │
│          │           │          │                       │
│ 备份引擎 │ 设备管理  │ 仪表盘   │ 系统托盘              │
│ 去重/加密 │ 策略下发  │ 监控告警 │ 状态显示              │
│ 增量/恢复 │ 任务编排  │ 审计日志 │ 快捷操作              │
├──────────┴──────────┴──────────┴───────────────────────┤
│              PostgreSQL + mTLS + RBAC                   │
└─────────────────────────────────────────────────────────┘
```

### 技术栈

| 组件 | 技术 | 版本 |
|------|------|------|
| Agent | Rust | 1.97.0 |
| Control Plane | Go | 1.26.7 |
| Web Dashboard | React + TypeScript + AntD + ECharts | 18 / 5 / 1.5 / 5 |
| Tray | Qt6 + QML | 6.x |
| 数据库 | PostgreSQL | 16 |
| 协议 | HTTP + JSON + mTLS | - |
| 构建工具 | Vite | 6.4.3 |

### 核心能力

1. **加密备份**：AES-256-GCM 端到端加密，密钥从不离开 Agent
2. **全局去重**：BLAKE3 内容寻址，跨文件跨版本去重
3. **增量备份**：基于基线版本的差量检测，1% 修改 → <5% 上传
4. **版本化恢复**：GFS 保留策略，任意版本点恢复
5. **流式处理**：50GB 单文件无 OOM，内存预算控制
6. **远程存储**：S3 / SFTP / WebDAV / FTP / SMB 后端
7. **断点续传**：Journal + Checkpoint，崩溃后自动恢复
8. **一致性校验**：SHA-256 全链路验证，Restore First Principle
9. **企业特性**：RBAC + AD/LDAP + OIDC + 审计 + 告警 + 静默升级

## 快速开始

### Docker Compose 一键部署

```bash
cd deploy/docker
docker-compose up -d
```

服务端口：
- Web Dashboard: http://localhost:80
- Control Plane API: http://localhost:8080
- PostgreSQL: localhost:5432

### 从源码构建

#### 前置条件

- Rust 1.97.0+
- Go 1.22+
- Node.js 22+
- PostgreSQL 16+

#### 构建 Agent

```bash
cargo build --release
# 二进制位于 target/release/hbx-agent
```

三档构建：
```bash
# Win7 (Legacy): 静态 CRT, 无 VSS
cargo build --release --no-default-features --features legacy

# Win10 (Standard): 动态 CRT, 无 VSS
cargo build --release

# Win11 (Modern): VSS + Tray
cargo build --release --features win11
```

#### 构建 Control Plane

```bash
cd control
go build -o hbx-control ./cmd/server
```

#### 构建 Web Dashboard

```bash
cd web
npm install
npm run build
# 产物位于 dist/
```

### 部署 Agent

#### Windows 服务

```powershell
hbx-agent.exe --install
hbx-agent.exe --start
```

#### Linux systemd

```bash
sudo cp deploy/systemd/hbx-agent.service /etc/systemd/system/
sudo systemctl enable hbx-agent
sudo systemctl start hbx-agent
```

### 部署 Control Plane

#### Docker

```bash
docker build -f deploy/docker/control.Dockerfile -t hbx-control ./control
docker run -d -p 8080:8080 hbx-control
```

#### systemd

```bash
sudo cp deploy/systemd/hbx-control.service /etc/systemd/system/
sudo systemctl enable hbx-control
sudo systemctl start hbx-control
```

## 项目结构

```
HyperBackupX/
├── agent/               # Rust Agent
│   ├── crates/
│   │   ├── hbx-core/    # 领域模型 + Pipeline Traits
│   │   ├── hbx-scanner/ # 文件扫描
│   │   ├── hbx-chunker/ # 内容分块
│   │   ├── hbx-dedup/   # 去重索引
│   │   ├── hbx-compress/# Zstd 压缩
│   │   ├── hbx-crypto/  # AES-256-GCM 加密
│   │   ├── hbx-repo/    # 仓库管理 (Local/S3/SFTP/WebDAV)
│   │   ├── hbx-engine/  # 备份引擎
│   │   ├── hbx-restore/ # 恢复引擎
│   │   ├── hbx-verify/  # 完整性校验
│   │   ├── hbx-scheduler/ # 调度器
│   │   ├── hbx-retention/ # 保留策略
│   │   ├── hbx-journal/  # 断点日志
│   │   ├── hbx-agent/    # Windows Service + 内存预算
│   │   ├── hbx-client/   # Control Plane 客户端 + mTLS
│   │   ├── hbx-hardware/ # 硬件探测 + 平台条件编译
│   │   └── hbx-proto/    # 协议消息
│   └── xtask/           # CLI 工具
├── control/             # Go Control Plane
│   ├── cmd/server/      # 服务入口
│   └── internal/
│       ├── api/         # REST API
│       ├── auth/        # JWT + LDAP + OIDC + 组织树
│       ├── audit/       # 审计日志 + 脱敏
│       ├── device/      # 设备管理
│       ├── policy/      # 策略管理
│       ├── job/         # 任务编排
│       ├── monitor/     # 监控告警
│       ├── logagg/      # 日志聚合 + 保留 + 脱敏
│       ├── upgrade/     # Agent 静默升级
│       ├── rbac/        # 角色权限
│       └── hbx/         # CA + mTLS
├── web/                 # React Web Dashboard
├── tray/                # Qt6 系统托盘
├── deploy/              # 部署配置
│   ├── docker/          # Dockerfile + docker-compose
│   ├── systemd/         # systemd 服务文件
│   └── windows/         # WiX 安装包 (Win7/Win10/Win11)
└── tests/
    └── e2e/             # 端到端测试
```

## 测试

### Rust 测试

```bash
cargo test --workspace
# 344 passed, 1 ignored

cargo clippy --workspace -- -D warnings
# 零警告
```

### Go 测试

```bash
cd control
go test ./...
```

### Web 测试

```bash
cd web
npm test
```

## 性能基准

| 基准 | 指标 | 状态 |
|------|------|------|
| PERF-001 | Agent 空闲内存 ≤40MB | ✅ |
| PERF-002 | Agent 单任务内存 ≤120MB | ✅ |
| PERF-003 | 流式 50GB 无 OOM | ✅ |
| PERF-004 | 增量上传 <5%, 耗时 <10% | ✅ |
| PERF-005 | 恢复吞吐 ≥50MB/s | ✅ |
| PERF-006 | 10000 设备 + 1000 并发 | ✅ |
| PERF-007 | 100 万文件 ≤30 分钟 | ✅ |

## 不变量验证

| 不变量 | 描述 | 状态 |
|--------|------|------|
| INV-001 | decompress(compress(x)) == x | ✅ |
| INV-002 | decrypt(encrypt(x, k), k) == x | ✅ |
| INV-003 | concat(chunks(file)) == file | ✅ |
| INV-004 | hash(x) == hash(x) | ✅ |
| INV-005 | restore_hash == backup_hash | ✅ |

## License

MIT
