# 待完善功能实现计划

## 概述

根据功能对比分析，当前核心功能覆盖率已达 95%，还有 3 项功能需要完善：

| 功能 | 优先级 | 复杂度 | 预计工作量 |
|------|--------|--------|------------|
| 移动位置订阅 | 高 | 中 | 2-3小时 |
| 设备配置查询 | 中 | 中 | 1-2小时 |
| 日志管理完善 | 低 | 低 | 1小时 |

---

## 任务一：移动位置订阅完善

### 1.1 当前状态
- 端点：`GET /api/device/query/subscribe/mobile-position`
- 问题：仅返回成功，未发送 SIP SUBSCRIBE，未持久化订阅周期

### 1.2 实现步骤

#### 步骤 1：修改 handlers/device_stub.rs
```
文件：src/handlers/device_stub.rs
函数：subscribe_mobile_position

改动：
1. 解析请求参数（device_id, cycle, expires）
2. 调用 db::device::update_mobile_position_subscription 持久化订阅周期
3. 调用 sip_server.send_subscribe(device_id, "MobilePosition", cycle) 发送 SIP SUBSCRIBE
4. 返回实际结果（成功/失败）
```

#### 步骤 2：添加数据库函数
```
文件：src/db/device.rs
新增函数：
- update_mobile_position_subscription(pool, device_id, cycle) -> sqlx::Result<()>
- get_devices_for_mobile_position_renewal(pool) -> sqlx::Result<Vec<(String, i32)>>

数据库字段：
- wvp_device.subscribe_cycle_for_mobile_position (已存在)
```

#### 步骤 3：添加 SIP SUBSCRIBE 发送
```
文件：src/sip/server.rs
参考：send_subscribe_internal 函数（目录订阅已实现）

改动：
1. 在 run() 中添加移动位置订阅自动续期后台任务
2. 复用 send_subscribe_internal 发送 MobilePosition SUBSCRIBE
```

#### 步骤 4：处理 NOTIFY 响应
```
文件：src/sip/server.rs
函数：handle_notify

改动：
1. 解析 MobilePosition NOTIFY 消息
2. 提取经纬度信息
3. 更新设备通道的 longitude/latitude
4. 通过 WebSocket 广播位置更新
```

### 1.3 涉及文件
| 文件 | 操作 |
|------|------|
| `src/handlers/device_stub.rs` | 修改 subscribe_mobile_position |
| `src/db/device.rs` | 添加订阅持久化函数 |
| `src/sip/server.rs` | 添加后台续期任务、处理 NOTIFY |

---

## 任务二：设备配置查询完善

### 2.1 当前状态
- 端点：`GET /api/device/config/query/:device_id/BasicParam`
- 问题：返回空对象，未发送 SIP 查询命令

### 2.2 实现步骤

#### 步骤 1：修改 handlers/device_stub.rs
```
文件：src/handlers/device_stub.rs
函数：config_basic_param

改动：
1. 解析路径参数 device_id
2. 调用 sip_server.send_config_query(device_id, "BasicParam")
3. 等待响应或超时（使用 oneshot channel）
4. 返回配置参数
```

#### 步骤 2：添加 SIP ConfigQuery 发送
```
文件：src/sip/server.rs
新增函数：send_config_query

实现：
1. 构建 ConfigQuery XML 消息
2. 发送 SIP MESSAGE
3. 等待设备响应
4. 解析响应 XML 中的配置参数
```

#### 步骤 3：处理配置响应
```
文件：src/sip/server.rs
函数：handle_message

改动：
1. 识别 ConfigQuery 响应消息
2. 解析 BasicParam 配置
3. 通过 channel 返回给调用者
```

### 2.3 涉及文件
| 文件 | 操作 |
|------|------|
| `src/handlers/device_stub.rs` | 修改 config_basic_param |
| `src/sip/server.rs` | 添加 send_config_query、处理响应 |

---

## 任务三：日志管理完善

### 3.1 当前状态
- 端点：`GET /api/log/list`
- 问题：简单查询，缺少分页和过滤功能

### 3.2 实现步骤

#### 步骤 1：修改 handlers/stub.rs
```
文件：src/handlers/stub.rs
函数：log_list

改动：
1. 添加分页参数（page, count）
2. 添加时间范围过滤（start_time, end_time）
3. 添加日志类型过滤（type）
4. 添加查询关键字（query）
```

#### 步骤 2：修改数据库查询
```
文件：src/handlers/stub.rs

改动 SQL：
- 添加 WHERE 条件过滤
- 添加 LIMIT/OFFSET 分页
- 返回 total 总数
```

### 3.3 涉及文件
| 文件 | 操作 |
|------|------|
| `src/handlers/stub.rs` | 修改 log_list 函数 |

---

## 实现顺序

```
1. 日志管理完善（最简单，快速完成）
   └─> 验证编译

2. 设备配置查询完善
   └─> 验证编译

3. 移动位置订阅完善（最复杂）
   └─> 验证编译

4. 最终验证
   └─> cargo build --release
   └─> cargo build --release --no-default-features --features mysql
```

---

## 验证清单

- [ ] 日志管理分页过滤功能
- [ ] 设备配置查询真实实现
- [ ] 移动位置订阅真实实现
- [ ] PostgreSQL 编译通过
- [ ] MySQL 编译通过
