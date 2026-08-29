# HyperBackup X 运维部署指南

| 项目 | 值 |
|------|-----|
| 文档版本 | v1.0 |
| 软件版本 | badou-server 0.1.0 |
| 适用平台 | Debian 13 (trixie) |
| 日期 | 2026-08-29 |

---

## 1. 概述

八斗存储桶服务器 (`badou-server`) 是 HyperBackup X 的核心存储后端，提供基于 gRPC 的内容寻址存储服务。本指南涵盖服务器的安装、配置、启动和监控。

### 1.1 服务端口

| 端口 | 协议 | 用途 | 默认地址 |
|------|------|------|----------|
| 9090 | gRPC | 主存储服务 (HBOP 协议) | `0.0.0.0:9090` |
| 9091 | HTTP | Prometheus 指标端点 | `0.0.0.0:9091` |
| 9092 | HTTP | 管理 API | `0.0.0.0:9092` |

### 1.2 系统要求

| 项目 | 最低要求 | 推荐 |
|------|----------|------|
| OS | Debian 12+ | Debian 13 (trixie) |
| RAM | 2 GB | 6 GB |
| 磁盘 | 20 GB | 110 GB |
| Rust | 1.96.0 | 1.98.0 |
| 网络 | 100 Mbps | 1 Gbps |

---

## 2. 安装

### 2.1 环境准备

```bash
# 更新系统
sudo apt-get update && sudo apt-get upgrade -y

# 安装编译工具链
sudo apt-get install -y build-essential pkg-config libssl-dev

# 安装 Rust (如未安装)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 验证 Rust 版本
rustc --version  # 需要 >= 1.96.0
```

### 2.2 编译服务器

```bash
# 克隆仓库
git clone https://github.com/hyperbackupx/hbx.git
cd hbx

# 编译 badou-server (release 模式)
cargo build --release -p badou-server

# 编译产物
ls -la badou/target/release/badou-server
```

> **注意**: 编译需要约 6 GB RAM，首次编译约需 10-15 分钟。

### 2.3 目录结构

```
HyperBackupX/
├── badou/                    # 八斗存储桶 (服务端)
│   ├── crates/
│   │   ├── badou-server/     # 服务器入口
│   │   ├── badou-store/      # 存储引擎
│   │   ├── badou-engine/     # 备份引擎
│   │   ├── badou-proto/      # gRPC 协议定义
│   │   └── ...
│   └── target/release/       # 编译产物
├── agent/                    # HyperBackup X Agent (客户端)
│   ├── crates/
│   │   ├── hbx-cli/          # CLI 工具
│   │   ├── hbx-engine/       # 备份引擎
│   │   └── ...
│   └── target/release/       # 编译产物
└── tests/e2e/                # 端到端测试
```

---

## 3. 配置

### 3.1 配置文件

服务器通过 JSON 配置文件启动，默认路径为当前目录下的 `badou-server.json`，也可通过命令行参数指定。

```bash
# 使用默认配置文件
./badou-server

# 指定配置文件路径
./badou-server /path/to/badou-server.json
```

### 3.2 配置项说明

```json
{
    "data_root": "/tmp/badou-data",
    "bind_addr": "0.0.0.0:9090",
    "metrics_addr": "0.0.0.0:9091",
    "management_addr": "0.0.0.0:9092",
    "jwt_secret": "your-secret-key",
    "tls": null,
    "cluster": {
        "mode": "single"
    }
}
```

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `data_root` | string | 是 | — | 数据存储根目录 |
| `bind_addr` | string | 是 | — | gRPC 监听地址 |
| `metrics_addr` | string | 是 | — | Prometheus 指标监听地址 |
| `management_addr` | string | 否 | (无) | 管理 API 监听地址 |
| `jwt_secret` | string | 是 | — | JWT 签名密钥 (不能为空) |
| `tls` | object | 否 | `null` | TLS 配置 (见 3.4) |
| `cluster` | object | 是 | — | 集群配置 (见 3.5) |

### 3.3 单节点配置示例

```json
{
    "data_root": "/var/lib/badou",
    "bind_addr": "0.0.0.0:9090",
    "metrics_addr": "0.0.0.0:9091",
    "management_addr": "0.0.0.0:9092",
    "jwt_secret": "change-this-to-a-strong-secret",
    "cluster": {
        "mode": "single"
    }
}
```

### 3.4 TLS 配置

启用 mTLS 双向认证：

```json
{
    "data_root": "/var/lib/badou",
    "bind_addr": "0.0.0.0:9090",
    "metrics_addr": "0.0.0.0:9091",
    "jwt_secret": "your-secret",
    "tls": {
        "server_cert": "/etc/badou/tls/server.crt",
        "server_key": "/etc/badou/tls/server.key",
        "client_ca_cert": "/etc/badou/tls/client-ca.crt"
    },
    "cluster": {
        "mode": "single"
    }
}
```

### 3.5 集群配置

**单节点模式**:

```json
{
    "cluster": {
        "mode": "single"
    }
}
```

**Raft 集群模式**:

```json
{
    "cluster": {
        "mode": "raft",
        "node_id": "node-1",
        "peers": ["node-2:9090", "node-3:9090"]
    }
}
```

| 字段 | 说明 |
|------|------|
| `mode` | `single` 或 `raft` |
| `node_id` | 当前节点 ID (raft 模式必填) |
| `peers` | 集群对等节点列表 (raft 模式) |

### 3.6 配置验证

服务器启动时自动验证配置，以下情况会启动失败：

| 错误 | 原因 |
|------|------|
| `jwt_secret 不能为空` | `jwt_secret` 字段为空字符串 |
| `bind_addr 无效` | `bind_addr` 不是合法的 socket 地址 |
| `metrics_addr 无效` | `metrics_addr` 不是合法的 socket 地址 |
| `management_addr 无效` | `management_addr` 不是合法的 socket 地址 |

---

## 4. 启动与运行

### 4.1 前台启动

```bash
# 创建数据目录
mkdir -p /var/lib/badou

# 创建配置文件
cat > /etc/badou/server.json << 'EOF'
{
    "data_root": "/var/lib/badou",
    "bind_addr": "0.0.0.0:9090",
    "metrics_addr": "0.0.0.0:9091",
    "management_addr": "0.0.0.0:9092",
    "jwt_secret": "your-strong-secret",
    "cluster": { "mode": "single" }
}
EOF

# 前台启动
./badou-server /etc/badou/server.json
```

**启动日志**:

```
八斗存储桶服务器启动中...
数据目录: "/var/lib/badou"
gRPC 监听: 0.0.0.0:9090
Prometheus 指标: 0.0.0.0:9091
管理 API: 0.0.0.0:9092
集群模式: 单节点
八斗存储桶服务器就绪，等待连接...
```

### 4.2 后台启动 (nohup)

```bash
nohup ./badou-server /etc/badou/server.json > /var/log/badou/server.log 2>&1 &
echo $! > /var/run/badou.pid
```

### 4.3 systemd 服务

创建服务单元文件：

```bash
sudo cat > /etc/systemd/system/badou-server.service << 'EOF'
[Unit]
Description=HyperBackup X BaDou Storage Server
After=network.target

[Service]
Type=simple
User=badou
Group=badou
ExecStart=/opt/badou/badou-server /etc/badou/server.json
Restart=on-failure
RestartSec=5
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF
```

```bash
# 创建用户和目录
sudo useradd -r -s /bin/false badou
sudo mkdir -p /opt/badou /var/lib/badou /var/log/badou /etc/badou
sudo chown -R badou:badou /var/lib/badou /var/log/badou

# 复制二进制
sudo cp target/release/badou-server /opt/badou/

# 启用并启动
sudo systemctl daemon-reload
sudo systemctl enable badou-server
sudo systemctl start badou-server

# 查看状态
sudo systemctl status badou-server
```

### 4.4 优雅关闭

```bash
# systemd
sudo systemctl stop badou-server

# 手动 (发送 SIGINT/Ctrl+C)
kill -INT $(cat /var/run/badou.pid)
```

服务器收到 `Ctrl+C` 信号后会打印 `收到关闭信号 (Ctrl+C)，正在关闭...` 并优雅退出。

---

## 5. 网络与防火墙

### 5.1 开放端口

```bash
# 使用 iptables
sudo iptables -A INPUT -p tcp --dport 9090 -j ACCEPT  # gRPC
sudo iptables -A INPUT -p tcp --dport 9091 -j ACCEPT  # Prometheus
sudo iptables -A INPUT -p tcp --dport 9092 -j ACCEPT  # 管理 API

# 使用 ufw (如已安装)
sudo ufw allow 9090/tcp
sudo ufw allow 9091/tcp
sudo ufw allow 9092/tcp
```

### 5.2 安全建议

| 端口 | 暴露范围 | 建议 |
|------|----------|------|
| 9090 (gRPC) | 内网 | 仅对备份客户端开放 |
| 9091 (Prometheus) | 内网 | 仅对监控系统开放 |
| 9092 (管理 API) | 本地/内网 | 建议仅本地访问或通过 VPN |

---

## 6. 监控

### 6.1 Prometheus 指标

指标端点: `http://<server-ip>:9091/metrics`

```bash
# 手动查看指标
curl http://192.168.2.87:9091/metrics
```

**Prometheus scrape 配置示例**:

```yaml
scrape_configs:
  - job_name: 'badou-server'
    static_configs:
      - targets: ['192.168.2.87:9091']
    scrape_interval: 15s
```

### 6.2 健康检查

管理 API 健康检查端点: `http://<server-ip>:9092/health`

```bash
curl http://192.168.2.87:9092/health
# 响应: {"status":"healthy","data_root":"/var/lib/badou"}
```

### 6.3 日志

服务器使用 `tracing` 日志框架，日志输出到 stderr。可通过 `RUST_LOG` 环境变量控制日志级别：

```bash
# 设置日志级别
export RUST_LOG=info          # 默认
export RUST_LOG=debug         # 调试
export RUST_LOG=warn          # 仅警告及以上

# 启动时指定
RUST_LOG=debug ./badou-server /etc/badou/server.json
```

| 日志级别 | 说明 |
|----------|------|
| `error` | 仅错误 |
| `warn` | 警告及以上 |
| `info` | 信息及以上 (推荐) |
| `debug` | 调试及以上 |
| `trace` | 全部 |

---

## 7. 数据目录结构

```
/var/lib/badou/                          # data_root
├── repositories/
│   └── <repo-id>/
│       ├── chunks/                      # 内容寻址 chunk 存储
│       │   └── <shard-dir>/
│       │       └── <chunk-hash>.chunk   # chunk 数据文件
│       └── snapshots/
│           └── <snapshot-id>.json       # 快照元数据
└── ...
```

### 7.1 磁盘空间规划

| 数据类型 | 增长率 | 说明 |
|----------|--------|------|
| chunks | ~源数据大小 × 去重率 | 内容寻址，自动去重 |
| snapshots | ~每版本数 KB | 压缩存储 (zstd) |
| journal | ~备份中文件数 × 路径长度 | 临时文件，备份完成后可清理 |

**估算公式**: `磁盘需求 ≈ 源数据总量 × (1 - 去重率) + 版本数 × 100KB`

---

## 8. 验证部署

### 8.1 连接测试

```bash
# 检查端口监听
ss -tlnp | grep -E '909[012]'

# 检查 gRPC 端口
nc -zv 192.168.2.87 9090

# 检查管理 API
curl -s http://192.168.2.87:9092/health | jq .
```

### 8.2 功能测试

```bash
# 设置环境变量
export HBX_SERVER_URL=http://192.168.2.87:9090
export HBX_TOKEN=<your-jwt-token>

# CLI 连接测试
hbx-cli version

# 执行小型备份测试
hbx-cli backup <test-job-id> --repo <test-repo-id>

# 验证仓库
hbx-cli compat verify <test-repo-id> --mode Quick
```

### 8.3 端到端测试

```bash
# 运行 E2E 测试套件 (需要服务器运行中)
TMPDIR=/home/debian/tmp \
BADOU_E2E_ENDPOINT=http://127.0.0.1:9090 \
cargo test -p hbx-e2e-tests --lib -- --ignored --nocapture
```

---

## 9. 升级

### 9.1 升级步骤

```bash
# 1. 停止服务
sudo systemctl stop badou-server

# 2. 备份当前二进制和配置
sudo cp /opt/badou/badou-server /opt/badou/badou-server.bak
sudo cp /etc/badou/server.json /etc/badou/server.json.bak

# 3. 拉取新代码并编译
cd /path/to/hbx
git pull
cargo build --release -p badou-server

# 4. 替换二进制
sudo cp target/release/badou-server /opt/badou/

# 5. 启动服务
sudo systemctl start badou-server

# 6. 验证
curl -s http://localhost:9092/health | jq .
```

### 9.2 回滚

```bash
sudo systemctl stop badou-server
sudo cp /opt/badou/badou-server.bak /opt/badou/badou-server
sudo cp /etc/badou/server.json.bak /etc/badou/server.json
sudo systemctl start badou-server
```