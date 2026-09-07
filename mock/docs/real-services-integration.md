# GBServer 与真实服务集成指南

> 本文档说明 `mock/` 目录下脚本如何与**真实运行的** ZLM、Redis、PostgreSQL/MySQL/SQLite 协同工作。

## 1. 设计哲学

GBServer 的运行依赖：

| 依赖 | 真实 vs Mock | 说明 |
|------|--------------|------|
| **ZLMediaKit** | ✅ 真实 | 已有 `127.0.0.1:8080`（secret=`EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw`） |
| **Redis** | ✅ 真实 | 已有 `127.0.0.1:6379` |
| **PostgreSQL** | ⚠️ 本机无 | docker-compose.yml 提供；按需 `docker compose up -d postgres` |
| **MySQL** | ⚠️ 本机无 | docker-compose.yml `--profile mysql up -d` |
| **SQLite** | ✅ 默认 | `data/gbserver.db` 由 GBServer 自动建 |
| **GB28181 IPC/NVR** | ❌ 真实难弄 | mock `sip_device_mock.py` 替代 |
| **JT1078 部标终端** | ❌ 真实昂贵 | mock `jt1078_terminal_mock.py` 替代 |
| **上级 GB28181 平台** | ❌ 真实难找 | mock `cascade_mock.py` 替代 |
| **业务 Webhook 接收器** | 可选 | mock `webhook_receiver.py` 替代（用于验证回调） |

## 2. 自动发现

```bash
bash mock/scripts/discover-services.sh
```

输出示例（本机实测）：

```
==== 本机真实服务探测 ====

  ✅ GBServer  HTTP  http://127.0.0.1:18080/api/health
  ✅ ZLM (default secret)  HTTP  http://127.0.0.1:8080/index/api/getApiList?secret=...
  ✅ Redis  TCP  127.0.0.1:6379
  ❌ PostgreSQL  TCP  127.0.0.1:5432  未监听
  ❌ MySQL  TCP  127.0.0.1:3306  未监听
  ✅ SQLite   /Users/letmlook/code/GBServer/data/gbserver.db

==== 探测结果 ====
  GBServer HTTP : 运行中
  ZLM HTTP      : 运行中
  Redis TCP     : 运行中
  PostgreSQL    : 未运行
  MySQL         : 未运行
  SQLite 文件   : 存在
```

## 3. 启动策略

`start-mocks.sh` 已自动适配：

```bash
# 默认行为：发现 ZLM 真实可用 → 跳过 ZLM mock
bash mock/scripts/start-mocks.sh

# 强制启动 ZLM mock（覆盖真实 ZLM；用于测试 GBServer 处理 ZLM 异常）
bash mock/scripts/start-mocks.sh --force-mock-zlm
```

## 4. 种子数据

### 4.1 数据库种子

```bash
# 自动检测后端并灌入
bash mock/scripts/seed-database.sh

# 强制指定
bash mock/scripts/seed-database.sh sqlite
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/gbserver \
  bash mock/scripts/seed-database.sh postgres
DATABASE_URL=mysql://root:Fitow2022@127.0.0.1:3306/gbserver \
  bash mock/scripts/seed-database.sh mysql
```

种子内容（与实际 schema 对齐，参见 `seed/sqlite/seed-test-data.sql`）：
- **3 用户**：admin / viewer / tester（密码统一 `admin`）
- **3 区域**：北京市 / 海淀区 / 朝阳区
- **2 分组**：重点监控组 / 车载终端组
- **3 GB28181 设备**：测试摄像机-01/02、测试 NVR
- **4 通道**：每个设备的子通道
- **1 流媒体服务器**：`zlmediakit-1` 指向真实 ZLM
- **1 级联平台**：Mock-级联平台-1（指向 `cascade_mock.py`）
- **2 JT1078 终端**：京A12345 / 京B54321
- **3 报警**：视频丢失 / 磁盘满 / 超速

### 4.2 Redis 种子

```bash
# 优先用 redis-cli
bash mock/scripts/seed-redis.sh

# redis-cli 不可用时，备用 Python 实现（无需第三方库）
python3 mock/scripts/seed_redis.py

# 自定义 Redis
REDIS_URL=redis://:mypass@redis.example.com:6379/1 \
  python3 mock/scripts/seed_redis.py
```

种子内容（`seed/redis/seed-test-data.json`）：
- `state_store:*` — 节点状态
- `cache:*` — API 缓存示例
- `ratelimit:*` — 限流计数
- `session:*` — 用户会话
- `rpc:*` — 跨节点 RPC

## 5. GBServer 配置与真实服务对接

`config/application.toml` 中关键字段（已正确配置）：

```toml
[database]
url = "sqlite://data/gbserver.db?mode=rwc"
# url = "postgres://postgres:postgres@127.0.0.1:5432/gbserver"
# url = "mysql://root:Fitow2022@127.0.0.1:3306/gbserver"

[redis]
url = "redis://127.0.0.1:6379"

[[zlm.servers]]
id = "zlmediakit-1"
ip = "127.0.0.1"
http_port = 8080
secret = "EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw"  # ← 与真实 ZLM 对齐
enabled = true

[zlm]
hook_url = "http://127.0.0.1:18080/api/zlm/hook"

[jt1078]
retransmit_hook_url = "http://127.0.0.1:18080/api/jt1078/retransmit"
```

## 6. 端到端集成验证（本机实测）

```bash
# 1. 真实服务已在运行（GBServer / ZLM / Redis）
# 2. 灌入数据库种子
bash mock/scripts/seed-database.sh
# → 4 设备、4 通道、3 用户等

# 3. 灌入 Redis 种子
python3 mock/scripts/seed_redis.py
# → 11 个测试 key

# 4. 启动必要 mock（SIP / JT1078 / 级联 / Webhook）
bash mock/scripts/start-mocks.sh
# → 真实 ZLM 在 8080，skip；启动 sip-device / jt1078 / cascade / webhook-receiver

# 5. 验证全部联通
bash mock/scripts/check-services.sh
```

### 6.1 API 验证

```bash
JWT=$(curl -s "http://127.0.0.1:18080/api/user/login?username=admin&password=admin" \
  | python3 -c "import json,sys; print(json.load(sys.stdin)['data']['accessToken'])")

# 设备列表（应包含 4 台：GBServer + 3 个 mock 设备）
curl -s "http://127.0.0.1:18080/api/device/query/devices?page=1&count=10" \
  -H "access-token: $JWT" | jq '.data.list[].deviceId'

# 流媒体服务器列表（应包含 zlmediakit-1，online）
curl -s "http://127.0.0.1:18080/api/server/media_server/list" \
  -H "access-token: $JWT" | jq '.data[].id'

# JT1078 终端列表
curl -s "http://127.0.0.1:18080/api/jt1078/terminal/list" \
  -H "access-token: $JWT" | jq '.data'
```

### 6.2 协议层验证

```bash
# SIP 设备注册（自动）
tail -f mock/logs/sip-device.log
# 预期：REGISTER sent → 401 → REGISTER with Digest → 200 OK → 设备已注册 → 周期性 Keepalive

# JT1078 终端注册（自动）
tail -f mock/logs/jt1078-terminal.log
# 预期：TX msg_id=0x0100 → 注册成功 → TX msg_id=0x0002（心跳）

# 级联平台收到 REGISTER（GBServer 作为下级 → 模拟上级）
tail -f mock/logs/cascade-platform.log
# 预期：RX REGISTER → TX 200 OK
```

### 6.3 ZLM Hook 端到端

真实 ZLM **没有** mock 的 `/trigger/*` 端点。两种方式触发回调：

A. **真实推流**：用 FFmpeg 推一路流到真实 ZLM，ZLM 自动发 webhook 到 GBServer
```bash
ffmpeg -re -f lavfi -i testsrc=size=320x240:rate=10 \
  -f lavfi -i sine=frequency=1000:sample_rate=8000 \
  -c:v libx264 -c:a aac -f flv \
  rtmp://127.0.0.1:1935/live/test01
# ZLM → POST /api/zlm/hook (on_publish) → GBServer 收到
```

B. **直接 POST Webhook**：绕过 ZLM，直接构造 webhook 负载发给 GBServer
```bash
curl -X POST http://127.0.0.1:18080/api/zlm/hook \
  -H 'Content-Type: application/json' \
  -d '{
    "hook_name": "on_record_mp4",
    "mediaServerId": "zlmediakit-1",
    "schema": "rtsp",
    "app": "live",
    "stream": "test01",
    "vhost": "__defaultVhost",
    "file_name": "test.mp4",
    "file_path": "/tmp/test.mp4",
    "file_size": 1024,
    "file_duration": 60.0,
    "file_create_time": "2026-08-23 20:00:00"
  }'
# GBServer 应返回 200 + code:0；触发云录像入库逻辑
```

## 7. 检查清单

部署完 mock 套件后，确认：

- [ ] `bash mock/scripts/discover-services.sh` 显示 ZLM / Redis / SQLite ✅
- [ ] `bash mock/scripts/seed-database.sh` 成功灌入种子（无 SQL 错误）
- [ ] `python3 mock/scripts/seed_redis.py` 成功灌入 11 个 Redis key
- [ ] `bash mock/scripts/start-mocks.sh` 跳过真实 ZLM，启动 SIP/JT1078/cascade/webhook mock
- [ ] `tail mock/logs/sip-device.log` 显示 200 OK 与周期性 Keepalive
- [ ] `tail mock/logs/jt1078-terminal.log` 显示 ✅ 注册成功
- [ ] `curl http://127.0.0.1:18080/api/device/query/devices` 返回 ≥ 4 台设备
- [ ] `curl http://127.0.0.1:9090/received` 显示 Webhook 记录

## 8. 三数据库矩阵

```bash
# SQLite（默认）
bash mock/scripts/seed-database.sh sqlite

# PostgreSQL（启动容器）
docker compose up -d postgres
DATABASE_URL=postgres://postgres:postgrespw@127.0.0.1:5432/gbserver \
  bash mock/scripts/seed-database.sh postgres

# MySQL（启动容器）
docker compose --profile mysql up -d
DATABASE_URL=mysql://root:Fitow2022@127.0.0.1:3306/gbserver \
  bash mock/scripts/seed-database.sh mysql
```

每次切换数据库后，记得重启 GBServer 让其连接新数据库：
```bash
pkill -f "cargo run" || pkill -f gbserver
cargo run --release
```

## 9. 与现有 tests/ 套件的关系

| 套件 | 依赖 | 适合场景 |
|------|------|----------|
| `tests/`（Rust 集成测试） | wiremock / testcontainers / SQLite | CI 单元与集成测试，无外部依赖 |
| `e2e/`（Playwright UI 烟囱） | 前端 dev server + GBServer | UI 端到端验证 |
| **`mock/`（本目录）** | **真实 ZLM/Redis/DB + Python mock** | **手动 / 演示 / 集成 / 回归** |

三者互补：
- `tests/` 用于 PR / CI 自动验证
- `mock/` 用于开发期人工调试、演示、POC
- `e2e/` 用于 UI 变更验证