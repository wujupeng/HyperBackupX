# HyperBackup X 客户端使用手册

| 项目 | 值 |
|------|-----|
| 文档版本 | v1.0 |
| 软件版本 | hbx-cli 0.1.0 |
| 适用平台 | Debian 13 (trixie)、Windows 11 |
| 日期 | 2026-08-29 |

---

## 1. 概述

HyperBackup X 客户端 (`hbx-cli`) 是一个与 Duplicati 兼容的命令行工具，用于连接八斗存储桶服务器 (badou-server) 执行备份、恢复、验证等操作。客户端通过 gRPC 协议与服务器通信，支持 JWT 身份认证。

### 1.1 架构关系

```
hbx-cli (客户端)  ──gRPC──>  badou-server (八斗存储桶)  ──>  本地/分布式存储
     │                              │
     │                              ├── Prometheus 指标 (:9091)
     │                              └── 管理 API (:9092)
     │
     └── HTTP REST API (兼容模式)
```

### 1.2 支持的平台

| 平台 | 状态 | 备注 |
|------|------|------|
| Debian 13 (trixie) | ✅ 已验证 | 推荐平台 |
| Windows 11 | ✅ 已验证 | 路径使用反斜杠 |
| Windows 10 | ⚠️ 未测试 | 理论支持 |
| Windows 7 | ⚠️ 未测试 | 理论支持 |

---

## 2. 安装

### 2.1 从源码编译

**前置条件**: Rust ≥ 1.96.0

```bash
# 克隆仓库
git clone https://github.com/hyperbackupx/hbx.git
cd hbx

# 编译 CLI (release 模式)
cargo build --release -p hbx-cli

# 编译产物位于
# Linux:   target/release/hbx-cli
# Windows: target/release/hbx-cli.exe
```

### 2.2 验证安装

```bash
hbx-cli version
# 输出: hbx-cli 0.1.0 (HyperBackup X CLI)
```

### 2.3 环境变量配置

| 变量名 | 说明 | 默认值 | 必填 |
|--------|------|--------|------|
| `HBX_SERVER_URL` | 服务器 API 地址 | `http://localhost:8080` | 是 |
| `HBX_TOKEN` | JWT 认证令牌 | (无) | 是 |

**配置示例**:

```bash
# Linux (bash)
export HBX_SERVER_URL=http://192.168.2.87:9090
export HBX_TOKEN=your-jwt-token-here

# Windows (PowerShell)
$env:HBX_SERVER_URL = "http://192.168.2.87:9090"
$env:HBX_TOKEN = "your-jwt-token-here"
```

---

## 3. 命令参考

### 3.1 命令总览

```
hbx-cli <command> [subcommand] [options]
```

| 命令 | 说明 |
|------|------|
| `compat backup` | 触发兼容模式备份 |
| `compat restore` | 恢复指定版本 |
| `compat list` | 列出作业、版本或文件 |
| `compat delete` | 删除指定版本 |
| `compat verify` | 验证仓库完整性 |
| `compat import` | 导入 Duplicati 配置 |
| `backup` | `compat backup` 的快捷方式 |
| `restore` | `compat restore` 的快捷方式 |
| `list` | `compat list` 的快捷方式 |
| `import` | `compat import` 的快捷方式 |
| `help` | 显示帮助信息 |
| `version` | 显示版本信息 |

### 3.2 备份 — `compat backup`

**语法**:

```
hbx-cli compat backup <job-id> --repo <repo-id>
```

**参数**:

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `<job-id>` | 位置参数 | 是 | 备份作业 ID (UUID 格式) |
| `--repo` | 选项 | 是 | 目标仓库 ID (UUID 格式) |

**示例**:

```bash
hbx-cli compat backup 550e8400-e29b-41d4-a716-446655440000 \
  --repo 660e8400-e29b-41d4-a716-446655440000
```

**输出**:

```
Backup triggered successfully: {...}
```

### 3.3 恢复 — `compat restore`

**语法**:

```
hbx-cli compat restore <version-id> --target <path> [--selection <rule>] [--mode <mode>]
```

**参数**:

| 参数 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `<version-id>` | 位置参数 | 是 | — | 要恢复的版本 ID |
| `--target` | 选项 | 是 | — | 恢复目标路径 |
| `--selection` | 选项 | 否 | `all` | 文件选择规则 |
| `--mode` | 选项 | 否 | `original` | 恢复模式 |

**`--selection` 取值**:

| 值 | 说明 |
|----|------|
| `all` | 恢复全部文件 |
| `include:<patterns>` | 仅恢复匹配的文件 (分号分隔) |
| `exclude:<patterns>` | 排除匹配的文件 (分号分隔) |

**`--mode` 取值**:

| 值 | 说明 |
|----|------|
| `original` | 恢复到原始路径结构 |
| `overwrite` | 覆盖目标路径已有文件 |
| `merge` | 合并到目标路径 |

**示例**:

```bash
# 恢复全部文件
hbx-cli compat restore 770e8400-e29b-41d4-a716-446655440000 \
  --target /restore/path

# 仅恢复特定文件
hbx-cli compat restore 770e8400-e29b-41d4-a716-446655440000 \
  --target /restore/path \
  --selection "include:*.txt;*.doc"

# 排除特定文件并覆盖
hbx-cli compat restore 770e8400-e29b-41d4-a716-446655440000 \
  --target /restore/path \
  --selection "exclude:*.tmp" \
  --mode overwrite
```

> **已知限制**: 当前恢复操作仅使用文件名，不保留原始目录结构。子目录中的文件会被平铺到目标目录。

### 3.4 列出 — `compat list`

**语法**:

```
hbx-cli compat list <repo-id> [--versions] [--files <version>]
```

**参数**:

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `<repo-id>` | 位置参数 | 是 | 仓库 ID |
| `--versions` | 布尔标志 | 否 | 列出该仓库的所有版本 |
| `--files <version>` | 选项 | 否 | 列出指定版本的文件列表 |

**示例**:

```bash
# 列出仓库的兼容作业
hbx-cli compat list 660e8400-e29b-41d4-a716-446655440000

# 列出所有版本
hbx-cli compat list 660e8400-e29b-41d4-a716-446655440000 --versions

# 列出特定版本的文件
hbx-cli compat list 660e8400-e29b-41d4-a716-446655440000 \
  --files 770e8400-e29b-41d4-a716-446655440000
```

### 3.5 删除 — `compat delete`

**语法**:

```
hbx-cli compat delete <version-id> --force
```

**参数**:

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `<version-id>` | 位置参数 | 是 | 要删除的版本 ID |
| `--force` | 布尔标志 | 是 | 确认删除 (安全防护) |

**示例**:

```bash
hbx-cli compat delete 770e8400-e29b-41d4-a716-446655440000 --force
```

### 3.6 验证 — `compat verify`

**语法**:

```
hbx-cli compat verify <repo-id> [--mode <Quick|Full|Deep>]
```

**参数**:

| 参数 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `<repo-id>` | 位置参数 | 是 | — | 仓库 ID |
| `--mode` | 选项 | 否 | `Quick` | 验证模式 |

**验证模式**:

| 模式 | 说明 |
|------|------|
| `Quick` | 快速校验 (仅检查元数据) |
| `Full` | 完整校验 (校验所有 chunk 哈希) |
| `Deep` | 深度校验 (完整校验 + 数据一致性) |

**示例**:

```bash
hbx-cli compat verify 660e8400-e29b-41d4-a716-446655440000 --mode Full
```

### 3.7 导入配置 — `compat import`

**语法**:

```
hbx-cli compat import <config-file> [--dry-run]
```

**参数**:

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `<config-file>` | 位置参数 | 是 | Duplicati 配置文件路径 (JSON) |
| `--dry-run` | 布尔标志 | 否 | 仅解析预览，不实际导入 |

**示例**:

```bash
# 预览导入
hbx-cli compat import duplicati-config.json --dry-run

# 实际导入
hbx-cli compat import duplicati-config.json
```

**`--dry-run` 输出示例**:

```
Config name: MyBackup
Sources (3):
  - /home/user/documents
  - /home/user/photos
  - /home/user/projects
Destination: {"path": "/backup/dest"}
Encryption: {"method": "AES-256"}

Dry-run complete. No changes made.
```

---

## 4. 典型工作流

### 4.1 首次备份

```bash
# 1. 设置环境变量
export HBX_SERVER_URL=http://192.168.2.87:9090
export HBX_TOKEN=$(curl -s http://192.168.2.87:9092/health | jq -r '.token')

# 2. 触发备份
hbx-cli backup <job-id> --repo <repo-id>

# 3. 查看版本
hbx-cli list <repo-id> --versions
```

### 4.2 增量备份

增量备份使用相同的 `backup` 命令，引擎自动检测文件变更：

```bash
# 首次全量备份
hbx-cli backup <job-id> --repo <repo-id>

# 后续增量备份 (相同命令，引擎自动识别变更)
hbx-cli backup <job-id> --repo <repo-id>
```

### 4.3 恢复流程

```bash
# 1. 列出可用版本
hbx-cli list <repo-id> --versions

# 2. 查看版本文件列表
hbx-cli list <repo-id> --files <version-id>

# 3. 恢复到指定路径
hbx-cli restore <version-id> --target /restore/path

# 4. 验证恢复结果
diff -r /original/path /restore/path
```

### 4.4 从 Duplicati 迁移

```bash
# 1. 导出 Duplicati 配置为 JSON
# (在 Duplicati UI 中导出)

# 2. 预览导入
hbx-cli import duplicati-config.json --dry-run

# 3. 确认无误后导入
hbx-cli import duplicati-config.json

# 4. 使用导入的作业执行备份
hbx-cli backup <imported-job-id> --repo <repo-id>
```

---

## 5. 性能参考

以下数据基于 Phase BD-22 测试结果 (Debian 13, 6GB RAM, badou-server release build):

| 操作 | 数据量 | 耗时 | 吞吐率 |
|------|--------|------|--------|
| 全量备份 | 100 MB | 2.38s | 50 MB/s |
| 全量备份 | 1 GB | 15.56s | 66 MB/s |
| 全量备份 | 10 GB | 44.89s | 224 MB/s |
| 增量备份 | 100 MB (少量变更) | 2.59s | — |
| 恢复 | 100 MB | 9.98s | — |
| 断网恢复 | 280 MB (123 文件) | 4.11s | 68 MB/s |

**Windows 11 参考**:

| 操作 | 数据量 | 耗时 |
|------|--------|------|
| 全量备份 | 100 MB | 4.58s |
| 恢复 | 100 MB | 41.95s |

---

## 6. 错误处理

### 6.1 常见错误

| 错误信息 | 原因 | 解决方案 |
|----------|------|----------|
| `connection refused` | 服务器未运行或地址错误 | 检查 `HBX_SERVER_URL` 和服务器状态 |
| `unauthorized` | JWT 令牌无效或过期 | 重新获取令牌并设置 `HBX_TOKEN` |
| `missing <job-id> argument` | 缺少必填参数 | 查看命令帮助 `hbx-cli help` |
| `delete requires --force` | 删除操作未加 `--force` | 添加 `--force` 标志 |
| `CHUNK_NOT_FOUND` | 服务器上 chunk 缺失 | 执行 `compat verify --mode Full` 检查 |

### 6.2 断网恢复

备份过程中网络中断时，客户端会自动写入检查点日志 (journal)。网络恢复后重新执行备份命令即可从断点续传：

```bash
# 网络中断后，直接重新执行同一命令
hbx-cli backup <job-id> --repo <repo-id>
# 引擎自动从 journal 恢复，跳过已备份的文件
```

---

## 7. 帮助与版本

```bash
# 显示帮助
hbx-cli help
hbx-cli --help
hbx-cli -h

# 显示版本
hbx-cli version
hbx-cli --version
hbx-cli -V
```