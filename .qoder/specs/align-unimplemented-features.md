# 对齐未完全实现的功能

## Context

前期对比发现当前 Rust 后端有 10 个功能模块与 WVP Java 后端未完全对齐。经过深入探索，**云端录像任务(task/add, task/list)和 FFmpeg 命令模板列表已经有真实实现**，实际需要工作的是以下 8 项。用户要求：Redis 完整集成，负载均衡使用最少连接数策略。

---

## 修改项清单

### 1. 电子地图功能补全
**问题**: map_save_level/reset_level/thin_* 均为 stub；map_config 返回空对象
**数据库**: `wvp_device_channel` 已有 `map_level` 列

**修改文件**:
- `src/db/common_channel.rs` — 添加 map_level 更新/重置函数
- `src/handlers/common_channel.rs` — 实现 map_save_level/reset_level/thin_* 真实逻辑
- `src/handlers/server.rs` — map_config 返回实际地图配置
- `src/config.rs` — 添加 MapConfig (可选的 tianditu_key, center, zoom 等)

**具体改动**:
```
db/common_channel.rs:
+ update_map_level(pool, channel_ids, level) — 批量更新通道 map_level
+ reset_map_level(pool) — 将所有通道 map_level 重置为 0
+ update_channel_thin_data(pool, id, geojson) — (如需抽稀结果持久化)

handlers/common_channel.rs:
  map_save_level: 调用 db 更新 map_level
  map_reset_level: 调用 db 重置 map_level
  map_thin_clear/save/draw: 对通道坐标进行抽稀计算(内存操作)
  map_thin_progress: 返回实际抽稀进度(使用 Arc<AtomicU32>)

handlers/server.rs:
  map_config: 从 config 读取 MapConfig 返回 (tianditu_key, center, zoom, coord_sys)

config.rs:
+ MapConfig { tianditu_key, center_lng, center_lat, zoom, coord_sys }
  AppConfig.map 字段 (Optional)
```

### 2. 推流上传端点
**问题**: 前端调用 `/api/push/upload` 但后端无此端点

**修改文件**:
- `src/handlers/stream.rs` — 添加 push_upload 函数
- `src/router.rs` — 添加路由

**具体改动**:
```
handlers/stream.rs:
+ push_upload(State, Multipart) -> Result<Json<WVPResult<Value>>, AppError>
  - 使用 axum::extract::Multipart 接收文件
  - 保存到 uploads/ 目录
  - 创建推流记录到 DB
  - 返回 { app, stream, url }

router.rs:
+ .route("/api/push/upload", post(stream::push_upload))

Cargo.toml:
  axum features 添加 "multipart" (检查是否已有)
```

### 3. 报警订阅与持久化
**问题**: SIP 报警消息仅记录日志不入库；无主动报警订阅机制

**数据库**: `wvp_device_alarm` 表已存在于 schema

**修改文件**:
- `src/db/alarm.rs` (**新文件**) — 报警 CRUD
- `src/db/mod.rs` — 导出 alarm 模块
- `src/sip/server.rs` — handle_alarm 中写入数据库 + WebSocket 广播
- `src/handlers/device_stub.rs` — subscribe_catalog 旁边添加 subscribe_alarm 真实实现(如需)

**具体改动**:
```
db/alarm.rs (新建):
+ struct DeviceAlarm { id, device_id, channel_id, alarm_priority, alarm_method, alarm_type, alarm_time, alarm_description, longitude, latitude, handled, handle_time, handle_user, create_time }
+ insert_alarm(pool, alarm) -> sqlx::Result<u64>
+ list_alarms_paged(pool, device_id, channel_id, alarm_type, handled, start_time, end_time, page, count)
+ count_alarms(pool, filters...)
+ get_alarm_by_id(pool, id)
+ handle_alarm(pool, id, user, now)
+ delete_alarm(pool, id)

sip/server.rs handle_alarm():
  - 解析 XML 中的 AlarmPriority, AlarmMethod, AlarmType, AlarmTime, AlarmDescription, Longitude, Latitude
  - 调用 db::alarm::insert_alarm 入库
  - 通过 ws_state.broadcast() 发送报警事件到 WebSocket 客户端

注: handlers/alarm.rs 已有完整的 HTTP 查询端点(list/detail/handle/delete)，
    它们当前已查询 wvp_device_alarm 表，只需确保 SIP 端将数据写入此表即可。
```

### 4. Redis 缓存集成
**问题**: RedisConfig 已定义但无 redis 依赖、无连接初始化、无使用

**修改文件**:
- `Cargo.toml` — 添加 redis 依赖
- `src/lib.rs` — 初始化 Redis 连接并加入 AppState
- `src/db/mod.rs` 或 新建 `src/cache.rs` — Redis 缓存操作封装

**具体改动**:
```
Cargo.toml:
+ redis = { version = "0.25", features = ["aio", "tokio-comp", "connection-manager"] }

lib.rs:
  AppState 添加: pub redis: Option<redis::aio::ConnectionManager>
  run() 中: 如配置了 redis url，则创建 ConnectionManager 并存入 state

cache.rs (新建):
+ set_device_online(redis, device_id, online: bool, ttl_secs)
+ get_device_online(redis, device_id) -> Option<bool>
+ set_stream_info(redis, stream_key, info_json, ttl_secs)
+ get_stream_info(redis, stream_key) -> Option<String>
+ incr_media_server_streams(redis, server_id) -> i64
+ decr_media_server_streams(redis, server_id) -> i64
+ get_media_server_stream_count(redis, server_id) -> i64
  (以上函数在 redis 不可用时 graceful fallback，不影响核心功能)

使用点:
- sip/server.rs handle_register: set_device_online
- sip/server.rs handle_keepalive: 更新 device TTL
- handlers/play.rs play_start/stop: incr/decr stream count
- handlers/playback.rs playback_start/stop: incr/decr stream count
```

### 5. 多节点负载均衡 (最少连接数)
**问题**: 始终使用第一个 ZLM 节点，无自动分配

**修改文件**:
- `src/lib.rs` — 修改 get_zlm_client，添加 select_least_loaded_server
- `src/zlm/client.rs` — 添加 get_active_stream_count 方法
- `src/cache.rs` — Redis 中维护各节点流计数

**具体改动**:
```
lib.rs AppState:
  get_zlm_client(media_server_id):
    如果 media_server_id == "auto" 或 None:
      调用 select_least_loaded() 选择最少连接数的节点
    否则按 ID 查找

+ async fn select_least_loaded(&self) -> Option<(String, Arc<ZlmClient>)>
  - 如有 Redis: 从 Redis 读取各节点 stream count，选最小值
  - 如无 Redis: 遍历 zlm_clients，调用 get_media_list() 查询实际流数，选最小值
  - 缓存结果 5 秒避免频繁查询

zlm/client.rs:
+ get_active_stream_count() -> Result<usize>
  调用 /index/api/getMediaList 返回流数量

使用点:
- handlers/play.rs play_start: 使用 select_least_loaded 而非硬编码
- handlers/playback.rs: 同理
- sip/server.rs send_play_invite: 同理
```

### 6. 设备录像控制完善
**问题**: 录像控制为 fire-and-forget，无状态跟踪

**修改文件**:
- `src/handlers/device_stub.rs` — 完善 control_record

**具体改动**:
```
handlers/device_stub.rs control_record():
  现有实现已通过 SIP 发送 Record/StopRecord 命令
  补充:
  - 在 AppState 中添加 recording_state: Arc<RwLock<HashMap<String, RecordingInfo>>>
    RecordingInfo { device_id, channel_id, cmd, started_at }
  - start 时记录状态，stop 时清除
  - 通过 WebSocket 广播录像状态变更

  或更轻量方案: 将录像状态存入 Redis (如已集成):
  + set_recording_state(redis, device_id, channel_id, cmd)
  + get_recording_state(redis, device_id, channel_id) -> Option<String>
```

### 7. 目录订阅自动续期
**问题**: subscribe_catalog 为 stub，不持久化、不发送 SIP、无自动续期

**修改文件**:
- `src/handlers/device_stub.rs` — 实现 subscribe_catalog 真实逻辑
- `src/sip/server.rs` — 添加目录订阅自动续期后台任务
- `src/db/device.rs` — 确认 update_device_catalog_subscription 可用

**具体改动**:
```
handlers/device_stub.rs subscribe_catalog():
  - 调用 db::device::update_device_catalog_subscription(pool, device_id, cycle) 持久化
  - 调用 sip_server.send_subscribe(device_id, "Catalog", cycle) 发送 SIP SUBSCRIBE
  - 返回实际结果 (成功/失败)

sip/server.rs:
  添加后台任务 (tokio::spawn, 每 60s 检查一次):
  + catalog_subscription_renewal_task():
    - 查询所有 subscribe_cycle_for_catalog > 0 的在线设备
    - 检查其订阅是否即将过期 (剩余 < 30s)
    - 对即将过期的设备重新发送 SUBSCRIBE
    - 更新 CatalogSubscriptionManager 中的时间戳
```

### 8. 服务器地图配置 (合并到第1项)
已在第1项中处理: map_config 将从配置文件读取地图配置。

---

## 涉及的所有文件

| 文件 | 操作 | 内容 |
|------|------|------|
| `Cargo.toml` | 修改 | 添加 redis, axum multipart feature |
| `src/config.rs` | 修改 | 添加 MapConfig |
| `src/lib.rs` | 修改 | AppState 添加 redis, recording_state; select_least_loaded |
| `src/cache.rs` | **新建** | Redis 缓存操作封装 |
| `src/db/alarm.rs` | **新建** | 报警表 CRUD |
| `src/db/mod.rs` | 修改 | 导出 alarm, cache 模块 |
| `src/db/common_channel.rs` | 修改 | 添加 map_level 更新/重置函数 |
| `src/handlers/common_channel.rs` | 修改 | 实现地图 stub 函数 |
| `src/handlers/server.rs` | 修改 | map_config 返回真实配置 |
| `src/handlers/stream.rs` | 修改 | 添加 push_upload |
| `src/handlers/device_stub.rs` | 修改 | 实现 subscribe_catalog, 完善 control_record |
| `src/router.rs` | 修改 | 添加 push/upload 路由 |
| `src/sip/server.rs` | 修改 | handle_alarm 入库+广播; 添加目录订阅续期任务 |
| `src/zlm/client.rs` | 修改 | 添加 get_active_stream_count |

## 实现顺序

1. **Redis 集成** (Cargo.toml, config.rs, lib.rs, cache.rs) — 后续功能依赖
2. **多节点负载均衡** (lib.rs, zlm/client.rs, cache.rs) — 使用 Redis 计数
3. **报警订阅与持久化** (db/alarm.rs, sip/server.rs)
4. **目录订阅自动续期** (device_stub.rs, sip/server.rs)
5. **电子地图功能** (db/common_channel.rs, handlers/common_channel.rs, handlers/server.rs, config.rs)
6. **推流上传** (handlers/stream.rs, router.rs)
7. **设备录像控制完善** (handlers/device_stub.rs)

## 验证

1. `cargo build --release` 确认编译通过
2. `cargo build --release --no-default-features --features mysql` 确认 MySQL 编译通过
3. 启动服务验证:
   - Redis 连接成功 (日志输出)
   - `/api/server/map/config` 返回非空配置
   - `/api/common/channel/map/save-level` 写入数据库
   - `/api/device/query/subscribe/catalog` 发送 SIP SUBSCRIBE
   - 模拟报警 SIP MESSAGE → `wvp_device_alarm` 表有数据
   - `/api/push/upload` 上传文件成功
4. `cd web && npm run lint` 前端无报错
