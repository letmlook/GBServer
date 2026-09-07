# GBServer 模拟测试方案（TEST_PLAN）

> 本文档描述如何用 `mock/` 目录下的工具与种子数据，对 GBServer **全部公开功能** 做系统化测试。
> **核心策略：真实环境优先** —— ZLM / Redis / 数据库全部用真实运行的实例，只 mock 真实环境难弄的部分（SIP 设备、JT1078 终端、上级平台、Webhook）。

---

## 1. 测试目标与覆盖范围

### 1.1 目标

1. **零硬件依赖**：在没有真实相机 / 车载终端 / 上级平台的环境下完整跑通 GBServer
2. **真实集成**：ZLM / Redis / 数据库 用真实运行实例；mock 仅填补真实难弄的部分
3. **全链路**：覆盖 HTTP API、SIP 信令、JT1078 协议、ZLM Hook、Webhook 回调、前端 SPA
4. **多数据库**：SQLite（默认） / PostgreSQL / MySQL 三种后端种子一致
5. **回归安全**：每次接口变更后能用固定 mock 行为快速冒烟

### 1.2 服务分层

| 类别 | 服务 | 真实 vs Mock | 地址 |
|------|------|--------------|------|
| **后端核心** | GBServer HTTP API | ✅ 真实（cargo run） | `http://127.0.0.1:18080` |
| | GBServer SIP | ✅ 真实 | `udp://127.0.0.1:5060` |
| | GBServer JT1078 | ✅ 真实 | `udp://127.0.0.1:60000` |
| **流媒体** | ZLMediaKit | ✅ **真实** | `http://127.0.0.1:8080` |
| **数据库** | SQLite | ✅ 真实（自动建） | `data/gbserver.db` |
| | PostgreSQL | 可选真实（docker compose up） | `postgres://127.0.0.1:5432` |
| | MySQL | 可选真实（docker compose --profile mysql） | `mysql://127.0.0.1:3306` |
| **缓存** | Redis | ✅ **真实** | `redis://127.0.0.1:6379` |
| **外部协议端** | GB28181 IPC/NVR | ❌ Mock 必需 | `tools/sip-device` |
| | JT1078 部标终端 | ❌ Mock 必需 | `tools/jt1078-terminal` |
| | 上级 GB28181 平台 | ❌ Mock 必需 | `tools/cascade-platform` |
| **回调** | 业务 Webhook 接收 | 可选 Mock | `tools/webhook-receiver` |

### 1.3 占位 / Stub 标注（已知未实装）

参考 `src/handlers/stub.rs` 与 `device_stub.rs`，以下端点当前返回**兼容性空响应**。测试只验证响应**结构**，不验证业务正确性：

- `device_stub.rs`：17 个端点（设备 sync/delete/transport/subscribe_alarm/control_record/sub_channels/...）
- `stub.rs`：~50 个端点（region / group / log / userApiKey / cloud_record / record_plan / position_history / role/all / common_channel/list）

完整列表见 [docs/test-cases.md](./docs/test-cases.md) 附录 A。

---

## 2. 模拟工具对照表

| 真实依赖 | 协议 | mock 实现 | 替代能力 |
|----------|------|-----------|----------|
| GB28181 IPC / NVR | SIP UDP | `tools/sip-device/sip_device_mock.py` | REGISTER、200 OK、401 鉴权失败、心跳、Catalog 多包、DeviceInfo、Invite 200 OK + SDP、PTZ Info 响应 |
| JT1078 部标终端 | UDP/TCP | `tools/jt1078-terminal/jt1078_terminal_mock.py` | 0x0100 注册、0x0002 心跳、0x0102 应答占位、0x0801 录像列表、0x8300 文本响应、0x1006-0x100B 报警、0x0200 位置上报、0x0005 重传 |
| 上级 GB28181 平台 | SIP REGISTER | `tools/cascade-platform/cascade_mock.py` | 接收 GBServer 作为下级的 REGISTER + 心跳 + 目录查询 |
| 业务 Webhook 接收方 | HTTP POST | `tools/webhook-receiver/webhook_receiver.py` | 接收 JT1078 重传 / 云录像 / 业务回调 |
| **ZLMediaKit** | HTTP API | **真实 ZLM** | `getServerConfig` / `addStreamProxy` / `openRtpServer` / 74 个 API |
| **Redis** | RESP | **真实 Redis** | StateStore / Cache / RateLimit / Session |
| **PostgreSQL** | SQL | **真实 PG**（如已启动） | 多实例 StateStore / 跨节点 RPC |
| **MySQL** | SQL | **真实 MySQL**（如已启动） | 平迁兼容 |
| **SQLite** | SQL | **真实 SQLite**（默认） | 开发 / 演示 / 单机生产 |

> **重要**：`tools/zlm/zlm_mock.py` **仅在 ZLM 真实不可用时** 才需要启动；如启动真实 ZLM，加 `--force-mock-zlm` 强制覆盖。

---

## 3. 测试分级

| 级别 | 范围 | 自动化 | 运行成本 |
|------|------|--------|----------|
| L1 单元 | handler 函数、SIP 消息解析、JT1078 帧解析 | `cargo test` | 秒级 |
| L2 模块 | 单 mock + 单端点 | `cargo test` + Python 脚本 | 秒~分钟 |
| L3 集成 | 真实 ZLM/Redis/DB + 多 mock 联动 | `scripts/run-all-tests.sh` | 分钟 |
| L4 E2E | 完整协议流（SIP 注册→目录→Invite→播放→BYE） | Playwright + mock 编排 | 分钟 |
| L5 兼容 | 三数据库矩阵（SQLite / PG / MySQL） | CI matrix | 小时 |

---

## 4. 环境准备

### 4.1 探测真实服务

```bash
bash mock/scripts/discover-services.sh
```

输出示例：
```
==== 本机真实服务探测 ====
  ✅ GBServer  HTTP  http://127.0.0.1:18080/api/health
  ✅ ZLM (default secret)  HTTP  http://127.0.0.1:8080/index/api/getApiList?secret=...
  ✅ Redis  TCP  127.0.0.1:6379
  ❌ PostgreSQL  TCP  127.0.0.1:5432  未监听
  ❌ MySQL  TCP  127.0.0.1:3306  未监听
  ✅ SQLite   /Users/letmlook/code/GBServer/data/gbserver.db
```

### 4.2 启动 GBServer（任选数据库）

```bash
cd /Users/letmlook/code/GBServer
cargo run                                      # SQLite 默认（已就绪）
cargo run --no-default-features --features postgres
cargo run --no-default-features --features mysql
```

### 4.3 灌入种子数据

```bash
# 数据库（自动检测后端）
bash mock/scripts/seed-database.sh
# 输出示例：
#   ==== 后端: sqlite ====
#   → 灌入 .../seed/sqlite/seed-test-data.sql 到 data/gbserver.db

# Redis（优先 redis-cli，备用 Python 实现）
bash mock/scripts/seed-redis.sh       # 用 redis-cli
python3 mock/scripts/seed_redis.py    # 用 Python（无需 redis-cli）
```

### 4.4 启动 mock 套件

```bash
bash mock/scripts/start-mocks.sh
# 自动检测真实 ZLM → 跳过 ZLM mock
# 启动 SIP device / JT1078 terminal / cascade platform / webhook receiver
```

### 4.5 验证

```bash
bash mock/scripts/check-services.sh
```

---

## 5. 模拟工具详解

### 5.1 SIP 设备模拟器（`tools/sip-device/`）

**协议**：GB/T 28181-2016，SIP over UDP（15060）

**支持的行为**：
- `register`：发送 REGISTER，收到 401 后带 Digest 重新发送
- `keepalive`：周期性 MESSAGE（Notify/Keepalive），间隔 30 秒
- `unregister`：发送 Expires: 0
- `catalog`：接收 SUBSCRIBE / MESSAGE Catalog 请求，**分多包**返回（默认 8 个/包）
- `device_info`：响应 DeviceInfo 查询
- `device_status`：响应 DeviceStatus 查询
- `invite`：响应 Invite 请求，发送 200 OK + SDP，等待 ACK，3 秒后发送 BYE
- `ptz`：响应 MANSRTSP PTZ Info 命令

**启动**：
```bash
cd mock/tools/sip-device
python3 sip_device_mock.py \
  --server 127.0.0.1:5060 \
  --device-id 34020000001320000001 \
  --username admin \
  --password admin123 \
  --channels 4 \
  --auto-register \
  --auto-keepalive 30
```

### 5.2 JT1078 终端模拟器（`tools/jt1078-terminal/`）

**协议**：JT/T 1078-2016，UDP（16000）

**支持的消息**：
| 消息 ID | 名称 | 模拟器实现 |
|---------|------|-----------|
| 0x0100 | 注册 | ✅ |
| 0x0101 | 注册应答 | ✅ |
| 0x0002 | 心跳 | ✅ |
| 0x0001 | 通用应答 | ✅ |
| 0x0102 | 实时音视频 | ⚠️ 仅应答占位 |
| 0x0801 | 录像列表查询应答 | ✅ |
| 0x8300 | 文本信息下发 | ✅ 应答 |
| 0x1006-0x100B | 报警 | ✅ 上报 |
| 0x0200 | 位置上报 | ✅（模拟北京三环附近移动） |
| 0x0005 | 重传请求 | ✅ 按 SEQ 重发 |

**特殊能力**：
- `--simulate-loss 0.1`：10% 丢包率（触发后端重传检测）
- `--simulate-reorder 0.2`：20% 乱序（占位）

### 5.3 ZLMediaKit（**真实**，非 mock）

```bash
# 探测真实 ZLM
curl -s "http://127.0.0.1:8080/index/api/getServerConfig?secret=EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw"

# 列出现有流
curl -s "http://127.0.0.1:8080/index/api/getMediaList?secret=EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw"

# 触发 GBServer Webhook 处理：直接推流到 ZLM
ffmpeg -re -f lavfi -i testsrc=size=320x240:rate=10 \
  -f lavfi -i sine=frequency=1000:sample_rate=8000 \
  -c:v libx264 -c:a aac -f flv \
  rtmp://127.0.0.1:1935/live/test01
```

### 5.4 备选 ZLM mock（`tools/zlm/`）

仅当真实 ZLM 不可用时启用：
```bash
bash mock/scripts/start-mocks.sh --force-mock-zlm
# 或单独启：
python3 tools/zlm/zlm_mock.py --secret demo --hook-url http://127.0.0.1:18080/api/zlm/hook
```

实现 13 个 API + 6 个 `/trigger/*` 触发器，详见 [docs/zlm-api-reference.md](./docs/zlm-api-reference.md)。

### 5.5 上级平台模拟器（`tools/cascade-platform/`）

**协议**：SIP UDP 5062，**接收** GBServer 作为下级的 REGISTER

```bash
cd mock/tools/cascade-platform
python3 cascade_mock.py --port 5062 --server-id 34020000002000000099
```

然后通过 GBServer HTTP API 添加该平台：
```bash
curl -X POST http://127.0.0.1:18080/api/platform/add \
  -H "access-token: $JWT" \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "Mock-级联平台-1",
    "serverGBId": "34020000002000000099",
    "serverIp": "127.0.0.1",
    "serverPort": 5062,
    "deviceGBId": "34020000001320000001"
  }'
```

### 5.6 Webhook 接收器（`tools/webhook-receiver/`）

**协议**：HTTP POST，监听 9090

```bash
cd mock/tools/webhook-receiver
python3 webhook_receiver.py --port 9090 --log mock/logs/webhook.log
```

接收端点：`/hook/*`（任意路径）、`/received`（查询已接收列表）、`/received/clear`（清空）。

---

## 6. 测试运行步骤

### 6.1 启动完整测试环境

```bash
# 1. 后端（已在运行或新启动）
cd /Users/letmlook/code/GBServer && cargo run &

# 2. 灌入种子
bash mock/scripts/seed-database.sh
python3 mock/scripts/seed_redis.py

# 3. 启动 mock
bash mock/scripts/start-mocks.sh

# 4. 验证
bash mock/scripts/check-services.sh
```

### 6.2 核心场景

| 场景 | 命令 | 预期 |
|------|------|------|
| 健康检查 | `curl -s http://127.0.0.1:18080/api/health` | `{"status":"alive"}` |
| ZLM 联通 | `curl -s "http://127.0.0.1:8080/index/api/getServerConfig?secret=EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw"` | ZLM 配置 JSON |
| SIP 设备自动注册 | `tail -f mock/logs/sip-device.log` | 200 OK（可能 401 后再注册） |
| 触发 ZLM on_publish | 用 FFmpeg 推流到真实 ZLM | GBServer 收到 webhook |
| 模拟 Webhook（不走 ZLM） | `curl -X POST http://127.0.0.1:18080/api/zlm/hook -d '{...}'` | GBServer 处理 |
| JT1078 终端注册 | `tail -f mock/logs/jt1078-terminal.log` | `✅ 注册成功` |
| 重传 Webhook | 直接 POST `/api/jt1078/retransmit` 或触发 ZLM 录像完成 | `mock/logs/webhook.log` 有记录 |
| 实时播放 | `curl http://127.0.0.1:18080/api/play/start/...` | 触发 SIP Invite 到 mock |
| 设备列表（验证种子） | `curl 'http://127.0.0.1:18080/api/device/query/devices?page=1&count=10'` | 4 台设备（含 mock 摄像机） |

### 6.3 与 Cargo test 一起跑

```bash
cargo test --test integration_test
cargo test --test jt1078_integration
cargo test --test jt1078_e2_e2_test
cargo test --test integration
```

### 6.4 与 Playwright E2E 一起跑

```bash
cd e2e
npm install && npx playwright install chromium
# 确保后端 + mock 都在跑
npx playwright test
```

---

## 7. 验收标准

| 级别 | 通过条件 |
|------|----------|
| 单元 / 模块 | 0 panic / 0 未处理错误 |
| 集成 | HTTP 响应 `code:0`；DB 数据与期望一致；mock 日志显示注册成功 |
| E2E | 注册 → 目录 → Invite → 播放 → 停止 完整日志 |
| 兼容 | 三数据库下 `gb_device` 行数一致 |

---

## 8. 故障排查速查表

| 现象 | 可能原因 | 解决 |
|------|----------|------|
| SIP 注册 401 死循环 | Digest 密码不一致 | 检查 `config/application.toml` `[sip] password` 与 mock `--password` |
| ZLM 报 secret 错 | secret 不匹配 | `--secret EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw`（与 config 一致） |
| JT1078 心跳超时 | 防火墙拦截 UDP 60000 | `sudo lsof -iUDP:60000` |
| Webhook 没收到 | GBServer 未启或 URL 错 | `curl /api/health` 与 `--hook-url` |
| 数据库迁移报错 | schema 版本不一致 | `rm data/gbserver.db && cargo run` |
| Playwright 超时 | 后端/mock 未起 | 先跑 §6.1 |
| seed SQL 失败 | 表/列名变化 | 用 `sqlite3 data/gbserver.db ".schema <table>"` 比对 |

---

## 9. 附录

- A. 已知占位 / Stub 端点清单（见 [docs/test-cases.md](./docs/test-cases.md)）
- B. 完整 SIP 协议流程（见 [docs/sip-protocol-flow.md](./docs/sip-protocol-flow.md)）
- C. 完整 ZLM API 子集（mock 版）（见 [docs/zlm-api-reference.md](./docs/zlm-api-reference.md)）
- D. **真实服务对接指南**（见 [docs/real-services-integration.md](./docs/real-services-integration.md)）