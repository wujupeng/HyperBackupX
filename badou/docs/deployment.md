# BaDou 部署指南

## 1. 系统要求

- Debian 13 (Trixie) 或兼容 Linux 发行版
- PostgreSQL 15+ (用于 Control Plane 管理表)
- Rust 1.75+ (仅构建时需要)
- 无需 Kubernetes / Ceph / Kafka / Elasticsearch

## 2. 单节点部署

### 2.1 构建

```bash
cd badou
cargo build --release -p badou-server
cargo build --release -p badou-cli
```

### 2.2 安装

```bash
sudo ./deploy/install.sh
```

安装脚本将：
- 创建 `badou` 系统用户（非 root 运行）
- 安装二进制到 `/usr/local/bin/`
- 创建配置目录 `/etc/badou/`
- 创建数据目录 `/var/lib/badou/`
- 安装 systemd 服务单元
- 生成默认配置 `/etc/badou/server.json`

### 2.3 配置

编辑 `/etc/badou/server.json`：

```json
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
    "cert_path": "/etc/badou/tls/server.crt",
    "key_path": "/etc/badou/tls/server.key",
    "ca_path": "/etc/badou/tls/ca.crt"
  },
  "jwt": {
    "secret": "your-secure-secret",
    "issuer": "badou"
  }
}
```

### 2.4 启动

```bash
sudo systemctl start badou-server
sudo systemctl status badou-server
sudo journalctl -u badou-server -f
```

### 2.5 端口说明

| 端口 | 用途 |
|------|------|
| 50051 | HBOP gRPC (mTLS) |
| 9091  | Prometheus 指标 |

## 3. 多节点集群部署

### 3.1 初始化集群

在第一个节点上：

```bash
sudo ./deploy/cluster-init.sh node-1 192.168.1.60 50051 badou-cluster
```

### 3.2 加入新节点

在新节点上：

```bash
sudo ./deploy/install.sh
sudo ./deploy/cluster-join.sh node-2 192.168.1.61 50051 192.168.1.60 50051
```

### 3.3 验证集群

```bash
sudo badou-cli cluster status
sudo badou-cli health
```

## 4. mTLS 配置

### 4.1 生成证书

```bash
# CA
openssl req -x509 -newkey rsa:4096 -keyout ca.key -out ca.crt -days 3650 -nodes

# Server
openssl req -newkey rsa:4096 -keyout server.key -out server.csr -nodes
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -out server.crt -days 365

# Client
openssl req -newkey rsa:4096 -keyout client.key -out client.csr -nodes
openssl x509 -req -in client.csr -CA ca.crt -CAkey ca.key -out client.crt -days 365
```

### 4.2 安装证书

```bash
sudo mkdir -p /etc/badou/tls
sudo cp ca.crt server.crt server.key /etc/badou/tls/
sudo chown badou:badou /etc/badou/tls/*
sudo chmod 600 /etc/badou/tls/*.key
```

## 5. Control Plane 集成

### 5.1 数据库迁移

```bash
cd control
psql -f migrations/004_badou_tables.sql
```

### 5.2 注册八斗仓库

通过 Web Console 或 REST API：

```bash
curl -X POST http://control:8080/api/v1/badou/repositories \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"name":"badou-prod","node_address":"192.168.1.60","node_port":50051}'
```

## 6. 升级

```bash
# Build new version
cargo build --release -p badou-server

# Install
sudo ./deploy/install.sh

# Restart
sudo systemctl restart badou-server
```

systemd 的 `Restart=on-failure` 确保崩溃后自动重启，Journal 恢复机制保证数据一致性。