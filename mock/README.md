# GBServer 模拟测试资源（mock/）

> 与**真实运行**的 ZLM、Redis、PostgreSQL/MySQL/SQLite 协同工作的测试资源。
> 不需要完整硬件即可对 GBServer **全部功能** 做集成、E2E、手测验证。

---

## 📦 内容速览

| 目录 | 作用 |
|------|------|
| [`TEST_PLAN.md`](./TEST_PLAN.md) | **主文档**：测试矩阵、覆盖范围、运行步骤 |
| [`docs/real-services-integration.md`](./docs/real-services-integration.md) | **真实服务对接指南**（必读） |
| [`tools/`](./tools) | Python 模拟器（仅替代真实难弄的部分：SIP/JT1078/级联/Webhook） |
| [`seed/`](./seed) | 数据库 + Redis 种子数据 |
| [`fixtures/`](./fixtures) | 测试数据：HTTP 请求/响应、SIP 消息模板 |
| [`scripts/`](./scripts) | 一键启动 / 停止 / 检测 / 灌种子 |
| [`docker-compose.mock.yml`](./docker-compose.mock.yml) | 可选容器化 PG/MySQL/Redis |

---

## 🧰 模拟工具清单（仅替代真实难弄的部分）

| 工具 | 替代的真实依赖 | 协议 / 端口 | 真实环境已存在？ |
|------|----------------|------------|------------------|
| `sip-device/sip_device_mock.py` | GB28181 IPC / NVR | SIP UDP 15060 | ❌（真实环境无 IPC，必须 mock） |
| `jt1078-terminal/jt1078_terminal_mock.py` | JT1078 车辆终端 | UDP 16000 | ❌（真实环境无终端，必须 mock） |
| `zlm/zlm_mock.py` | ZLMediaKit | HTTP 8080 | ✅ **真实 ZLM 已在 127.0.0.1:8080 跑** |
| `cascade-platform/cascade_mock.py` | 上级 GB28181 平台 | SIP UDP 5062 | ❌（真实环境无上级平台，必须 mock） |
| `webhook-receiver/webhook_receiver.py` | 业务 Webhook 端点 | HTTP 9090 | （可选，用于验证回调） |

**关键差异（与上一版相比）**：ZLM 不再 mock，直接用真实 ZLM（secret=`EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw`）。`start-mocks.sh` 自动检测真实 ZLM 并跳过 mock。

## 📊 真实服务清单

| 服务 | 地址 | 用途 | 由谁提供 |
|------|------|------|----------|
| **GBServer HTTP API** | `http://127.0.0.1:18080` | 后端 | 用户运行 `cargo run` |
| **GBServer SIP** | `udp://127.0.0.1:5060` | 接收下级 SIP 设备 | GBServer 自身 |
| **GBServer JT1078** | `udp://127.0.0.1:60000` | 接收 JT1078 终端 | GBServer 自身 |
| **ZLMediaKit** | `http://127.0.0.1:8080` | 流媒体（secret=`EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw`） | 真实环境 |
| **Redis** | `redis://127.0.0.1:6379` | 缓存 / StateStore | 真实环境 |
| **SQLite** | `data/gbserver.db` | 默认数据库 | GBServer 自建 |
| **PostgreSQL** | `postgres://127.0.0.1:5432` | 可选生产数据库 | `docker compose up -d postgres` |
| **MySQL** | `mysql://127.0.0.1:3306` | 可选生产数据库 | `docker compose --profile mysql up -d` |

---

## 🚀 5 分钟上手

```bash
cd /Users/letmlook/code/GBServer

# 1. 探测真实服务
bash mock/scripts/discover-services.sh

# 2. 灌入测试种子（设备/通道/平台/JT1078/报警 + Redis state）
bash mock/scripts/seed-database.sh
python3 mock/scripts/seed_redis.py

# 3. 启动必要的 mock（SIP 设备 / JT1078 / 级联 / Webhook）
bash mock/scripts/start-mocks.sh
# 输出示例：
#   ==== 检测到真实 ZLM (127.0.0.1:8080) → 跳过 ZLM mock ====
#   [sip-device] starting...
#   [sip-device] ✅ started pid=... (UDP, no HTTP health)
#   [jt1078-terminal] starting...
#   [jt1078-terminal] ✅ started pid=... (UDP, no HTTP health)
#   [cascade-platform] starting...
#   [cascade-platform] ✅ started pid=... (UDP, no HTTP health)
#   [webhook-receiver] starting...
#   [webhook-receiver] ✅ started pid=...

# 4. 验证全部联通
bash mock/scripts/check-services.sh

# 5. 跑测试套件
bash mock/scripts/run-all-tests.sh --backend=sqlite
```

---

## 🎯 适用场景

1. **手工冒烟**：在真实 ZLM / Redis / DB 环境下，验证新接口端到端联通
2. **演示**：在没有真实相机 / 车载终端的环境演示完整功能
3. **集成测试**：覆盖 SIP / JT1078 协议与 HTTP API 的联动
4. **回归验证**：重构后用一组固定 mock 行为验证接口稳定性
5. **DB 种子重置**：测试前后快速恢复到一致的测试数据状态

---

## ⚠️ 与现有测试的关系

| 套件 | 适合场景 |
|------|----------|
| `tests/`（Rust 集成测试 + wiremock + testcontainers） | CI 自动验证，无外部依赖 |
| `e2e/`（Playwright UI 烟囱） | UI 端到端验证 |
| **`mock/`（本目录）** | **真实环境下的人工 / 集成 / 演示测试** |

本目录**不依赖** Rust 工具链，所有 mock 工具可用 Python 3.10+ 直接跑。

---

## 📚 文档索引

| 文档 | 内容 |
|------|------|
| [TEST_PLAN.md](./TEST_PLAN.md) | 测试矩阵 + 覆盖范围 + 运行步骤 |
| [docs/real-services-integration.md](./docs/real-services-integration.md) | **真实服务对接指南（必读）** |
| [docs/sip-protocol-flow.md](./docs/sip-protocol-flow.md) | GB28181 SIP 信令生命周期 |
| [docs/zlm-api-reference.md](./docs/zlm-api-reference.md) | ZLM 模拟实现的 API 子集 |
| [docs/test-cases.md](./docs/test-cases.md) | 详细测试用例与验收标准 |