# GBServer 部署方案指南

> **范围**：单机 / 多实例 / 数据库集群的判断阈值与部署模板。
>
> **数据库后端选型**（SQLite / PostgreSQL / MySQL）请参阅 [`DATABASE_COMPATIBILITY.md`](./DATABASE_COMPATIBILITY.md)。

---

## 1. 部署分级总览

| 级别 | 设备数 | 并发流 | 应用形态 | 数据库 | Redis | ZLM |
|------|--------|--------|----------|--------|-------|-----|
| **L1 演示 / 开发** | < 50 | < 10 | 单机 | SQLite | 无 | 1 |
| **L2 边缘节点** | < 200 | < 20 | 单机 | SQLite | 无 | 1 |
| **L3 小规模生产** | < 500 | < 50 | 单机 | SQLite / PG | 可选 | 1–2 |
| **L4 中等生产** | 500 ~ 2000 | 50 ~ 200 | 单机 | PG | 可选 | 2–3 |
| **L5 大规模生产** | 2000 ~ 5000 | 200 ~ 500 | 单机 + 主备 | **PG + Patroni** | 是 | 3+ |
| **L6 HA 集群** | > 2000 | > 200 | **多实例 + SIP LB** | **PG + Patroni** | **必选** | 3+ |
| **L7 超大规模** | > 5000 | > 500 | 多实例 + LB | **PG + Citus / TimescaleDB** | 是 | 5+ |
| **L8 WVP 平迁** | 任意 | 任意 | 单 / 多 | MySQL | 可选 | 任意 |

---

## 2. 决策流程

```
                          ┌─ WVP 平迁？ ─── 是 ──→ MySQL（任意级别）
                          │
开始选择数据库后端 ──┤
                          ├─ 设备 < 500 且无 HA 要求？ ── 是 ──→ SQLite
                          │
                          └─ 否则 ──→ PostgreSQL（推荐生产主力）
                                        │
                                        ├─ 设备 > 5000 或写入 > 1000 QPS？ ──→ DB 集群
                                        │
                                        └─ 否则 PG 单实例 + 备份

                          ┌─ 设备 > 2000 或并发流 > 200？ ── 是 ──→ 多实例 + Redis
                          │
判断应用层 ──┤
                          ├─ 设备 > 5000 或 SLA > 99.95%？ ── 是 ──→ DB 集群
                          │
                          └─ 否则 ──→ 单机（StateStore InMemory）
```

---

## 3. 单机部署（L1 ~ L5）

### 3.1 架构

```
┌──────────────────────────┐
│ GBServer (单进程)        │
│  ├─ SIP UDP/5060        │
│  ├─ HTTP :18080         │
│  ├─ JT1078 UDP          │
│  └─ StateStore (内存)   │
└──────────────────────────┘
       ↓
   PostgreSQL / MySQL / SQLite
```

### 3.2 特征

- 无 Redis：`StateStore::in_memory()` 自动启用
- 所有状态在进程内 DashMap
- 故障即停机，重启后设备需重新注册

### 3.3 适用场景

- 开发、演示、边缘节点
- 小规模生产（< 500 设备、可接受单点故障）

---

## 4. 多实例 + Redis（HA，L6）

### 4.1 架构

```
       ┌──────────────┐
       │ SIP LB /     │  (OpenSIPS / Kamailio)  ← 当前未实现
       │ UDP Proxy    │
       └──────┬───────┘
              │
   ┌──────────┴──────────┐
   ▼                     ▼
GBServer-A           GBServer-B
   │      └── StateStore (Redis) ──┘
   ▼
PostgreSQL (单实例 / Patroni)
```

### 4.2 关键依赖

| 依赖 | 必要性 | 当前状态 |
|------|--------|----------|
| Redis | **必须**（StateStore 跨节点共享） | ✅ 已实现 |
| SIP 负载均衡 | **必须**（UDP 端口复用 + dialog 转发） | ❌ **未实现**，HA 最大缺口 |
| PendingRequest 跨节点路由 | 必须 | ❌ 未实现 |
| SubscriptionLifecycle 分布式续期 | 必须 | ❌ 未实现 |
| JT1078 UDP 跨节点 | 必须 | ❌ 未实现 |

### 4.3 多实例已支持的部分

- ✅ 媒体服务器多节点（`[[zlm.servers]]` 配置）
- ✅ ZLM 负载均衡（Redis ZSET 最小连接数）
- ✅ 级联 SendRtp 跨节点（`cascade_forward.rs` StateStore 同步）

### 4.4 当前架构的真实 HA 瓶颈

1. **SIP dialog 持久化与转发** → 引入 OpenSIPS / Kamailio 做 SIP LB
2. **PendingRequest 跨节点路由** → StateStore 记录"请求来源节点 → 响应应回到节点 X"
3. **SubscriptionLifecycle 分布式续期** → Redis 驱动的调度队列
4. **JT1078 UDP 负载均衡** → L4 代理 + 全局 SSRC/电话号映射
5. **ZLMediaKit hook 白名单** → `hook.rs:406-412` 无 IP 校验，建议加 `[[zlm.hook_allowlist]]`

### 4.5 Redis 故障行为

参见 `OPERATIONS.md:207-213`：

> Redis 故障时 backend 自动回退 InMemoryBackend（每节点独立），跨节点状态会**发散**直至 Redis 恢复；对 invite/stream 状态可接受，对 sendrtp/recording 不推荐。

---

## 5. 数据库集群（L7）

### 5.1 触发条件（满足任一）

- [ ] 设备数 > 5000
- [ ] 位置 / 告警写入 > 1000 QPS
- [ ] 数据库可用性 SLA > 99.95%
- [ ] 跨地域多活
- [ ] 备份窗口 < 1 小时

### 5.2 PostgreSQL 集群方案

| 方案 | 适用规模 | 特性 |
|------|----------|------|
| 单实例 + WAL-G 备份 | < 3000 设备 | 简单，RPO ~5min |
| **Patroni + etcd** | 3000 ~ 10000 设备 | 自动故障切换，RPO < 1s |
| **Patroni + Citus** | > 10000 设备 | 水平分片，按 device_id 路由 |
| **TimescaleDB** | 高频位置 / 告警 | 时间分区，压缩存储 |

### 5.3 MySQL 集群方案

| 方案 | 适用规模 | 特性 |
|------|----------|------|
| 单实例 + xtrabackup | < 3000 设备 | 传统方案 |
| **MySQL Group Replication** | 3000 ~ 8000 设备 | 单主多写，Paxos |
| **PXC / Percona XtraDB** | 同上 | 同步复制，写性能略低 |
| **TiDB 兼容 MySQL** | > 10000 设备 | 水平扩展，HTAP |

### 5.4 SQLite 集群方案（**不推荐**）

SQLite 不原生支持集群，可选 LiteFS / rqlite / dqlite，均有限制。**设备 > 500 应直接迁到 PG/MySQL**，避免引入额外复杂度。

---

## 6. 完整部署模板

### 6.1 L1 演示 / 开发

```toml
# config/application.sqlite.toml
[database]
url = "sqlite://data/gbserver.db?mode=rwc"

[server]
port = 18080

[sip]
enabled = false

[zlm]
# 留空
```

```bash
cargo run --no-default-features --features sqlite
```

### 6.2 L3 小规模生产（SQLite + 单机）

```toml
[database]
url = "sqlite:///var/lib/gbserver/gbserver.db?mode=rwc"

[server]
port = 18080

[sip]
enabled = true
ip = "0.0.0.0"
port = 5060

[[zlm.servers]]
id = "zlm-1"
ip = "127.0.0.1"
http_port = 8080
secret = "your-zlm-secret"
enabled = true
```

备份策略：每日 `sqlite3 /var/lib/gbserver/gbserver.db ".backup /backup/gbserver-$(date +%F).db"`

### 6.3 L4 中等生产（PG + 单机）

```toml
[database]
url = "postgres://gbserver:***@10.0.1.10:5432/gbserver"

[server]
port = 18080

[sip]
enabled = true
ip = "10.0.1.20"
port = 5060

[redis]
url = "redis://10.0.1.11:6379/0"   # 可选，开启后可观测

[[zlm.servers]]
id = "zlm-1"
ip = "10.0.1.30"
http_port = 8080
secret = "***"
enabled = true

[[zlm.servers]]
id = "zlm-2"
ip = "10.0.1.31"
http_port = 8080
secret = "***"
enabled = true
```

### 6.4 L6 HA 集群（PG + Patroni + Redis + 多实例 + SIP LB）

```
外部负载均衡层：
  - OpenSIPS / Kamailio：SIP UDP/TCP LB
  - HAProxy / Nginx：HTTP API LB

应用层（无状态）：
  - GBServer-A (10.0.1.20) ← SIP LB → UDP/5060
  - GBServer-B (10.0.1.21) ← SIP LB → UDP/5060

数据层：
  - Redis Sentinel / Cluster（10.0.1.11~13）
  - PostgreSQL Patroni (10.0.1.15 primary + 10.0.1.16 replica + 10.0.1.17 replica)
  - etcd cluster (10.0.1.5~7)

媒体层：
  - ZLM-1 (10.0.1.30)
  - ZLM-2 (10.0.1.31)
  - ZLM-3 (10.0.1.32)
```

GBServer 启动参数：

```bash
GBSERVER__DATABASE__URL=postgres://gbserver:***@10.0.1.15:5432/gbserver \
GBSERVER__REDIS__URL=redis://10.0.1.11:6379/0 \
GBSERVER__SIP__IP=10.0.1.20 \
GBSERVER__RPC__NODE_ID=node-a \
GBSERVER__RPC__PEER_ENDPOINTS=["http://10.0.1.21:18080"] \
./gbserver
```

---

## 7. 配置参考

参见 `config/application.toml:1-100`：

| 配置项 | 含义 | 多实例关联 |
|--------|------|-----------|
| `redis.url` | Redis 连接字符串 | 设置后自动启用 RedisBackend，StateStore 跨节点共享 |
| `[rpc] node_id` | 本节点 ID | 多节点部署时唯一标识 |
| `[rpc] peer_endpoints` | 远端节点列表 | `["http://node2:18080"]` |
| `[[zlm.servers]]` | ZLM 媒体节点列表 | 多节点负载均衡，每节点独立健康检查 |
| `[sip] ip/port` | SIP 监听地址 | **单实例绑定**：UDP 5060 / TCP 5061，多实例需 SIP LB |
| `[sip] device_id` | 本级 GB-ID | 级联时作为平台身份标识 |
| `[jt1078]` | JT1078 会话参数 | 无多实例配置，端口由 OS 分配 |

---

## 8. SLA 与规模参考

参见 `OPERATIONS.md:17-220`：

| 指标 | 文档值 | 备注 |
|------|--------|------|
| RAM（单机） | 2GB 基础，4GB+ 对 500+ 设备 | |
| PostgreSQL 版本 | 13+，推荐 16 | |
| Redis | 6+，多节点必选 | |
| ZLMediaKit | 2024-01-01 构建版 | |

---

## 9. 监控与运维

| 维度 | 工具 | 备注 |
|------|------|------|
| 指标 | `/metrics` Prometheus 端点 | 当前已暴露 |
| 日志 | tracing + tracing-subscriber | env-filter |
| SIP 抓包 | sngrep / Homer | 排障必备 |
| ZLM 探活 | 健康检查循环（30s） | `health_checker.run_health_check_loop` |
| 数据库慢查询 | PG `pg_stat_statements` / MySQL `slow_query_log` | |

---

## 10. 升级与回滚

- **GBServer 滚动升级**：L6 集群可逐节点重启（前提：SipServer 已支持 dialog 持久化）
- **数据库迁移**：Phase 1 SQLite → Phase 2 PG：导出 SQL + 数据导入
- **配置回滚**：所有配置可通过 `GBSERVER__SECTION__KEY` 环境变量覆盖，无需改文件

---

## 11. 相关文件

- `config/application.toml` — 默认配置
- `docs/OPERATIONS.md` — 运维手册
- `src/state_store.rs:146-665` — InMemoryBackend / RedisBackend
- `src/sip/server.rs:347-351` — SIP 监听与端口独占
- `docker-compose.yml` — 默认依赖 PG / Redis