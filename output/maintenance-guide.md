# HyperBackup X 日常维护手册

| 项目 | 值 |
|------|-----|
| 文档版本 | v1.0 |
| 软件版本 | badou-server 0.1.0 / hbx-cli 0.1.0 |
| 适用平台 | Debian 13 (trixie) |
| 日期 | 2026-08-29 |

---

## 1. 概述

本手册涵盖 HyperBackup X 八斗存储桶服务器的日常运维操作，包括健康检查、数据完整性验证、垃圾回收、版本管理、故障排查和备份策略。

### 1.1 管理 API 端点

所有管理操作通过管理 API (默认端口 9092) 执行：

| 端点 | 方法 | 说明 |
|------|------|------|
| `/health` | GET | 健康检查 |
| `/api/v1/repos/{repo-id}/versions` | GET | 列出仓库版本 |
| `/api/v1/repos/{repo-id}/versions/{version-id}` | DELETE | 删除指定版本 |
| `/api/v1/repos/{repo-id}/verify` | POST | 触发仓库验证 |
| `/api/v1/repos/{repo-id}/gc` | POST | 触发垃圾回收 |

---

## 2. 日常健康检查

### 2.1 服务器健康状态

```bash
# 基本健康检查
curl -s http://192.168.2.87:9092/health | jq .
```

**正常响应**:

```json
{
    "status": "healthy",
    "data_root": "/var/lib/badou"
}
```

### 2.2 端口监听检查

```bash
# 检查三个端口是否在监听
ss -tlnp | grep -E '909[012]'

# 预期输出:
# LISTEN ... 0.0.0.0:9090 ...  # gRPC
# LISTEN ... 0.0.0.0:9091 ...  # Prometheus
# LISTEN ... 0.0.0.0:9092 ...  # 管理 API
```

### 2.3 进程状态检查

```bash
# systemd 管理
sudo systemctl status badou-server

# 手动管理
ps aux | grep badou-server
```

### 2.4 磁盘空间检查

```bash
# 检查数据目录磁盘使用
df -h /var/lib/badou

# 检查数据目录大小
du -sh /var/lib/badou/

# 检查各子目录大小
du -sh /var/lib/badou/repositories/*/
```

### 2.5 自动化健康检查脚本

```bash
#!/bin/bash
# health-check.sh - 八斗存储桶健康检查

SERVER="192.168.2.87"
MGMT_PORT=9092
ALERT_THRESHOLD=90  # 磁盘使用率告警阈值

# 1. 检查服务健康
health=$(curl -s http://${SERVER}:${MGMT_PORT}/health | jq -r '.status')
if [ "$health" != "healthy" ]; then
    echo "[CRITICAL] 服务器不健康: $health"
    exit 1
fi
echo "[OK] 服务器健康"

# 2. 检查端口
for port in 9090 9091 9092; do
    if ! nc -z ${SERVER} ${port} 2>/dev/null; then
        echo "[CRITICAL] 端口 ${port} 不可达"
        exit 1
    fi
done
echo "[OK] 所有端口可达"

# 3. 检查磁盘空间
disk_usage=$(df /var/lib/badou | tail -1 | awk '{print $5}' | tr -d '%')
if [ "$disk_usage" -gt "$ALERT_THRESHOLD" ]; then
    echo "[WARNING] 磁盘使用率 ${disk_usage}% 超过阈值 ${ALERT_THRESHOLD}%"
else
    echo "[OK] 磁盘使用率 ${disk_usage}%"
fi
```

---

## 3. 数据完整性验证

### 3.1 通过 CLI 验证

```bash
# 快速验证 (仅元数据)
hbx-cli compat verify <repo-id> --mode Quick

# 完整验证 (校验所有 chunk 哈希)
hbx-cli compat verify <repo-id> --mode Full

# 深度验证 (完整 + 数据一致性)
hbx-cli compat verify <repo-id> --mode Deep
```

### 3.2 通过管理 API 验证

```bash
# 触发仓库验证
curl -s -X POST http://192.168.2.87:9092/api/v1/repos/<repo-id>/verify | jq .
```

**响应示例**:

```json
{
    "repo_id": "660e8400-e29b-41d4-a716-446655440000",
    "passed": true,
    "total_checked": 15234,
    "total_failed": 0
}
```

### 3.3 验证结果解读

| 字段 | 说明 |
|------|------|
| `passed` | `true` = 全部通过, `false` = 有失败 |
| `total_checked` | 检查的 chunk 总数 |
| `total_failed` | 哈希不匹配的 chunk 数 |

**异常处理**:

- `total_failed > 0`: 存在数据损坏，需从备份恢复或重新备份相关文件
- `total_checked = 0`: 仓库为空或路径错误，检查 `repo-id` 是否正确

### 3.4 定期验证建议

| 验证模式 | 频率 | 说明 |
|----------|------|------|
| Quick | 每日 | 快速检查，适合 cron 定时 |
| Full | 每周 | 完整哈希校验 |
| Deep | 每月 | 深度一致性检查 |

**cron 配置示例**:

```bash
# 每日凌晨 3 点执行快速验证
0 3 * * * /opt/badou/hbx-cli compat verify <repo-id> --mode Quick >> /var/log/badou/verify.log 2>&1

# 每周日凌晨 4 点执行完整验证
0 4 * * 0 /opt/badou/hbx-cli compat verify <repo-id> --mode Full >> /var/log/badou/verify-full.log 2>&1
```

---

## 4. 版本管理

### 4.1 列出版本

```bash
# 通过管理 API
curl -s http://192.168.2.87:9092/api/v1/repos/<repo-id>/versions | jq .

# 通过 CLI
hbx-cli list <repo-id> --versions
```

**响应示例**:

```json
{
    "versions": [
        {
            "version_id": "v-001",
            "snapshot_id": "snap-001",
            "created_at": "2026-08-28T10:00:00Z",
            "size": 104857600,
            "chunk_count": 25,
            "status": "complete"
        }
    ]
}
```

### 4.2 删除版本

```bash
# 通过管理 API
curl -s -X DELETE http://192.168.2.87:9092/api/v1/repos/<repo-id>/versions/<version-id> | jq .

# 通过 CLI (需要 --force)
hbx-cli compat delete <version-id> --force
```

**响应示例**:

```json
{
    "deleted": true
}
```

### 4.3 版本保留策略

建议按照以下策略管理版本保留：

| 版本类型 | 保留数量 | 说明 |
|----------|----------|------|
| 日备份 | 7 | 最近 7 天 |
| 周备份 | 4 | 最近 4 周 |
| 月备份 | 12 | 最近 12 个月 |
| 年备份 | 3 | 最近 3 年 |

**清理脚本示例**:

```bash
#!/bin/bash
# retention-cleanup.sh - 版本保留清理

REPO_ID="<your-repo-id>"
SERVER="192.168.2.87:9092"
KEEP_DAILY=7

# 获取所有版本按时间排序
versions=$(curl -s http://${SERVER}/api/v1/repos/${REPO_ID}/versions \
    | jq -r '.versions | sort_by(.created_at) | reverse | .[] | "\(.version_id) \(.created_at)"')

# 保留最近 N 个，删除其余
count=0
echo "$versions" | while read version_id created_at; do
    count=$((count + 1))
    if [ "$count" -gt "$KEEP_DAILY" ]; then
        echo "Deleting old version: $version_id (created: $created_at)"
        curl -s -X DELETE http://${SERVER}/api/v1/repos/${REPO_ID}/versions/${version_id}
    fi
done
```

---

## 5. 垃圾回收

### 5.1 触发垃圾回收

```bash
# 通过管理 API
curl -s -X POST http://192.168.2.87:9092/api/v1/repos/<repo-id>/gc | jq .
```

**响应示例**:

```json
{
    "repo_id": "660e8400-e29b-41d4-a716-446655440000",
    "chunks_scanned": 15234,
    "chunks_deleted": 0,
    "bytes_freed": 0,
    "duration_ms": 0
}
```

### 5.2 GC 结果解读

| 字段 | 说明 |
|------|------|
| `chunks_scanned` | 扫描的 chunk 总数 |
| `chunks_deleted` | 删除的孤立 chunk 数 |
| `bytes_freed` | 释放的字节数 |
| `duration_ms` | GC 耗时 (毫秒) |

### 5.3 GC 执行建议

- **删除版本后**: 每次删除旧版本后执行 GC 清理孤立 chunk
- **定期执行**: 每周至少执行一次 GC
- **低峰期执行**: GC 会扫描全部 chunk，建议在业务低峰期执行

```bash
# cron: 每周日凌晨 5 点执行 GC
0 5 * * 0 curl -s -X POST http://192.168.2.87:9092/api/v1/repos/<repo-id>/gc >> /var/log/badou/gc.log 2>&1
```

---

## 6. 日志管理

### 6.1 日志查看

```bash
# systemd 日志
sudo journalctl -u badou-server -f          # 实时跟踪
sudo journalctl -u badou-server --since "1 hour ago"
sudo journalctl -u badou-server --since today

# 手动日志文件
tail -f /var/log/badou/server.log
```

### 6.2 日志级别调整

```bash
# 临时调整 (重启后失效)
sudo systemctl edit badou-server
# 添加:
# [Service]
# Environment="RUST_LOG=debug"
# 保存后:
sudo systemctl restart badou-server
```

### 6.3 日志轮转

创建 logrotate 配置：

```bash
sudo cat > /etc/logrotate.d/badou-server << 'EOF'
/var/log/badou/server.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 0644 badou badou
}
EOF
```

### 6.4 关键日志事件

| 日志内容 | 级别 | 说明 |
|----------|------|------|
| `八斗存储桶服务器就绪` | info | 服务器启动成功 |
| `收到关闭信号 (Ctrl+C)` | info | 优雅关闭开始 |
| `gRPC Server 错误` | error | gRPC 服务异常 |
| `Prometheus 指标端点错误` | error | 指标服务异常 |
| `管理 API 端点错误` | error | 管理 API 异常 |
| `读取 TLS 证书文件失败` | error | TLS 配置问题 |

---

## 7. 故障排查

### 7.1 服务器无法启动

| 症状 | 可能原因 | 排查方法 |
|------|----------|----------|
| `读取配置文件失败` | 配置文件路径错误或权限不足 | 检查路径和权限 |
| `解析配置文件失败` | JSON 格式错误 | 用 `jq .` 验证 JSON |
| `jwt_secret 不能为空` | jwt_secret 为空 | 设置非空密钥 |
| `bind_addr 无效` | 地址格式错误 | 确保格式 `IP:PORT` |
| 端口被占用 | 其他进程占用端口 | `ss -tlnp \| grep <port>` |

### 7.2 客户端连接失败

```bash
# 1. 检查网络连通性
ping 192.168.2.87
nc -zv 192.168.2.87 9090

# 2. 检查 HBX_SERVER_URL
echo $HBX_SERVER_URL

# 3. 检查 JWT 令牌
echo $HBX_TOKEN

# 4. 检查服务器日志
sudo journalctl -u badou-server --since "5 min ago"
```

### 7.3 备份失败

| 错误 | 可能原因 | 解决方案 |
|------|----------|----------|
| `CHUNK_NOT_FOUND` | chunk 在服务器上缺失 | 执行 `verify --mode Full` 检查 |
| `connection reset` | 网络中断 | 重新执行备份 (自动从 journal 恢复) |
| `timeout` | 网络延迟或服务器负载高 | 检查网络和服务器资源 |
| `unauthorized` | JWT 令牌过期 | 重新获取令牌 |

### 7.4 磁盘空间不足

```bash
# 1. 检查磁盘使用
df -h /var/lib/badou

# 2. 执行 GC 释放空间
curl -s -X POST http://192.168.2.87:9092/api/v1/repos/<repo-id>/gc | jq .

# 3. 删除旧版本
hbx-cli list <repo-id> --versions  # 查看版本
hbx-cli compat delete <old-version-id> --force

# 4. 再次执行 GC
curl -s -X POST http://192.168.2.87:9092/api/v1/repos/<repo-id>/gc | jq .

# 5. 确认空间释放
df -h /var/lib/badou
```

### 7.5 数据损坏

```bash
# 1. 执行完整验证
curl -s -X POST http://192.168.2.87:9092/api/v1/repos/<repo-id>/verify | jq .

# 2. 如有损坏 (total_failed > 0)
#    a. 确认损坏的 chunk 不影响最新版本
#    b. 从完好版本恢复数据
hbx-cli restore <good-version-id> --target /restore/path

#    c. 重新备份恢复的数据
hbx-cli backup <job-id> --repo <repo-id>
```

---

## 8. 性能监控

### 8.1 关键指标

通过 Prometheus 指标端点 (`http://<server-ip>:9091/metrics`) 监控：

```bash
curl -s http://192.168.2.87:9091/metrics
```

### 8.2 推荐监控项

| 监控项 | 说明 | 告警阈值 |
|--------|------|----------|
| 磁盘使用率 | `data_root` 分区使用率 | > 85% 警告, > 95% 严重 |
| gRPC 连接数 | 活跃连接数 | 根据容量设定 |
| 备份延迟 | 备份完成时间 | 超过基线 2 倍 |
| 验证失败 | `total_failed > 0` | 任何失败 |
| 服务状态 | 进程存活 | 不可达 |

### 8.3 Grafana 仪表板

建议创建 Grafana 仪表板监控以下面板：

1. **服务状态**: 健康状态、运行时间
2. **磁盘使用**: 数据目录大小、增长率
3. **备份性能**: 备份吞吐率、耗时趋势
4. **错误率**: gRPC 错误、验证失败

---

## 9. 备份策略建议

### 9.1 备份频率

| 数据类型 | 频率 | 保留 | 说明 |
|----------|------|------|------|
| 关键业务数据 | 每日 | 7 日 + 4 周 + 12 月 | 高频率、长保留 |
| 一般数据 | 每周 | 4 周 + 12 月 | 中等频率 |
| 归档数据 | 每月 | 12 月 + 3 年 | 低频率、超长保留 |

### 9.2 恢复演练

建议每月执行一次恢复演练：

```bash
# 1. 选择一个近期版本
hbx-cli list <repo-id> --versions

# 2. 恢复到临时目录
hbx-cli restore <version-id> --target /tmp/restore-test

# 3. 验证恢复数据
# (与源数据比对或执行应用级验证)

# 4. 清理测试数据
rm -rf /tmp/restore-test
```

### 9.3 灾难恢复

**服务器故障恢复流程**:

1. 在新服务器上安装 badou-server (参考部署指南)
2. 恢复 `data_root` 数据 (从磁盘备份/RAID/异地复制)
3. 恢复配置文件 (`/etc/badou/server.json`)
4. 启动服务并验证
5. 执行完整数据验证 (`verify --mode Full`)

---

## 10. 维护检查清单

### 10.1 每日检查

- [ ] 服务器健康状态 (`/health` 返回 healthy)
- [ ] 磁盘空间使用率 (< 85%)
- [ ] 前一日备份成功完成
- [ ] 无错误日志

### 10.2 每周检查

- [ ] 执行完整验证 (`verify --mode Full`)
- [ ] 执行垃圾回收 (GC)
- [ ] 检查版本保留策略执行情况
- [ ] 检查日志轮转

### 10.3 每月检查

- [ ] 执行深度验证 (`verify --mode Deep`)
- [ ] 执行恢复演练
- [ ] 检查性能趋势
- [ ] 审查备份策略是否满足需求
- [ ] 检查系统更新

### 10.4 每季度检查

- [ ] 审查并更新文档
- [ ] 评估容量增长趋势
- [ ] 检查安全配置 (JWT 密钥轮换、TLS 证书有效期)
- [ ] 灾难恢复全流程演练