# BaDou 运维手册

## 1. 健康检查

### 1.1 systemd 状态

```bash
sudo systemctl status badou-server
```

### 1.2 健康端点

```bash
sudo badou-cli health
```

### 1.3 Prometheus 指标

```bash
curl http://localhost:9091/metrics
```

关键指标：
- `badou_chunks_total` — Chunk 总数
- `badou_versions_total` — Version 总数
- `badou_gc_chunks_deleted` — GC 删除的 Chunk 数
- `badou_commit_duration_seconds` — Commit Backup 耗时
- `badou_restore_duration_seconds` — 恢复耗时

## 2. 扩缩容

### 2.1 添加节点

```bash
# 在新节点上
sudo ./deploy/install.sh
sudo ./deploy/cluster-join.sh node-N <new-addr> 50051 <leader-addr> 50051
```

### 2.2 移除节点

```bash
sudo badou-cli cluster remove <node-id>
```

或通过 REST API：

```bash
curl -X DELETE http://control:8080/api/v1/badou/cluster/nodes/<node-id> \
  -H "Authorization: Bearer <token>"
```

### 2.3 扩容磁盘

```bash
curl -X POST http://control:8080/api/v1/badou/cluster/capacity \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"node_id":"<node-id>","additional_bytes":107374182400}'
```

## 3. 故障恢复

### 3.1 崩溃恢复

BaDou 使用 append-only Journal 保证崩溃恢复。systemd `Restart=on-failure` 自动重启后：
1. 读取 Journal 最后一条 INCOMPLETE 记录
2. 回滚未完成的操作
3. 恢复到一致状态

### 3.2 手动恢复

```bash
sudo badou-cli recovery
```

### 3.3 节点故障

- **Follower 故障**：集群继续工作，自动重新复制
- **Leader 故障**：Raft 选举新 Leader，秒级切换
- **多节点故障**：需剩余节点 > (N/2 + 1) 维持可用

## 4. GC 管理

### 4.1 触发 GC

```bash
sudo badou-cli gc
```

或通过 REST API：

```bash
curl -X POST http://control:8080/api/v1/badou/repositories/<repo-id>/gc \
  -H "Authorization: Bearer <token>"
```

### 4.2 查看 GC 报告

```bash
curl http://control:8080/api/v1/badou/repositories/<repo-id>/gc/report \
  -H "Authorization: Bearer <token>"
```

## 5. 完整性校验

### 5.1 触发校验

```bash
sudo badou-cli verify
```

或通过 REST API：

```bash
curl -X POST http://control:8080/api/v1/badou/repositories/<repo-id>/verify \
  -H "Authorization: B, Bearer <token>" \
  -d '{"level":"full"}'
```

校验级别：
- **quick** — 仅校验元数据
- **full** — 校验元数据 + Chunk 哈希
- **deep** — 校验全部 + 可选重算

## 6. CLI 用法

```bash
# 初始化
badou-cli init --data-dir /var/lib/badou

# 校验
badou-cli verify --level full

# GC
badou-cli gc

# 健康检查
badou-cli health

# 集群状态
badou-cli cluster status

# 恢复
badou-cli recovery
```

## 7. 日志

### 7.1 systemd Journal

```bash
sudo journalctl -u badou-server -f
sudo journalctl -u badou-server --since "1 hour ago"
```

### 7.2 审计日志

所有关键操作（Commit/Delete/GC/Verify/Immutable/节点变更）自动记入 Control Plane 审计日志，通过 `/api/v1/audit` 查询。

## 8. 不可变保留

设置不可变保留期后，保留期内的 Version 不可删除：

```bash
curl -X POST http://control:8080/api/v1/badou/repositories/<repo-id>/immutable \
  -H "Authorization: Bearer <token>" \
  -d '{"retention_days":90}'
```