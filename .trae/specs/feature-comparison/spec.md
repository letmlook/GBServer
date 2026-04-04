# WVP GB28181 功能对比分析

## Why
对当前 Rust 后端与 WVP Java 后端进行全面功能对比，明确已完全实现、未完全实现、未添加的功能列表，为后续开发提供清晰指引。

## What Changes
- 整理已完全实现的功能模块
- 整理未完全实现（stub/占位）的功能模块
- 整理未添加的功能模块

---

## 一、已完全实现的功能模块

### 1. 用户管理模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| POST /api/user/login | 用户登录 | ✅ 完整实现 |
| GET /api/user/logout | 用户登出 | ✅ 完整实现 |
| GET/POST /api/user/userInfo | 用户信息 | ✅ 完整实现 |
| GET /api/user/users | 用户列表 | ✅ 完整实现 |
| POST /api/user/add | 添加用户 | ✅ 完整实现 |
| DELETE /api/user/delete | 删除用户 | ✅ 完整实现 |
| POST /api/user/changePassword | 修改密码 | ✅ 完整实现 |
| POST /api/user/changePasswordForAdmin | 管理员修改密码 | ✅ 完整实现 |
| POST /api/user/changePushKey | 修改推送Key | ✅ 完整实现 |

### 2. 设备管理模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/device/query/devices | 设备列表 | ✅ 完整实现 |
| GET /api/device/query/devices/:device_id | 设备详情 | ✅ 完整实现 |
| GET /api/device/query/devices/:device_id/channels | 通道列表 | ✅ 完整实现 |
| DELETE /api/device/query/devices/:device_id/delete | 删除设备 | ✅ 完整实现 |
| GET /api/device/query/devices/:device_id/sync | 同步设备 | ✅ 完整实现 |
| POST /api/device/query/transport/:device_id/:stream_mode | 传输模式 | ✅ 完整实现 |
| GET /api/device/query/sync_status | 同步状态 | ✅ 完整实现 |
| POST /api/device/query/device/add | 添加设备 | ✅ 完整实现 |
| POST /api/device/query/device/update | 更新设备 | ✅ 完整实现 |

### 3. 设备控制模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/device/control/ptz | 云台控制 | ✅ 完整实现 |
| GET /api/device/control/guard | 报警布防 | ✅ 完整实现 |
| GET /api/device/control/preset | 预置位 | ✅ 完整实现 |
| GET /api/device/control/record | 录像控制 | ✅ 完整实现（含状态跟踪） |
| GET /api/device/query/subscribe/catalog | 目录订阅 | ✅ 完整实现（含自动续期） |

### 4. 媒体服务模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/server/media_server/list | 服务器列表 | ✅ 完整实现 |
| GET /api/server/media_server/online/list | 在线服务器 | ✅ 完整实现 |
| GET /api/server/media_server/one/:id | 服务器详情 | ✅ 完整实现 |
| POST /api/server/media_server/save | 保存服务器 | ✅ 完整实现 |
| DELETE /api/server/media_server/delete | 删除服务器 | ✅ 完整实现 |
| GET /api/server/media_server/load | 负载均衡 | ✅ 完整实现（最少连接数） |

### 5. 实时视频模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/play/start/:device_id/:channel_id | 开始播放 | ✅ 完整实现 |
| GET /api/play/stop/:device_id/:channel_id | 停止播放 | ✅ 完整实现 |
| GET /api/play/broadcast/:device_id/:channel_id | 语音广播 | ✅ 完整实现 |
| GET /api/play/broadcast/stop/:device_id/:channel_id | 停止广播 | ✅ 完整实现 |

### 6. 回放模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/playback/start/:device_id/:channel_id | 开始回放 | ✅ 完整实现 |
| GET /api/playback/stop/:device_id/:channel_id/:stream_id | 停止回放 | ✅ 完整实现 |
| GET /api/playback/pause/:stream_id | 暂停回放 | ✅ 完整实现 |
| GET /api/playback/resume/:stream_id | 恢复回放 | ✅ 完整实现 |
| GET /api/playback/speed/:stream_id/:speed | 回放倍速 | ✅ 完整实现 |
| GET /api/gb_record/query/:device_id/:channel_id | 录像查询 | ✅ 完整实现 |
| GET /api/gb_record/download/start/:device_id/:channel_id | 开始下载 | ✅ 完整实现 |
| GET /api/gb_record/download/stop/:device_id/:channel_id/:stream_id | 停止下载 | ✅ 完整实现 |

### 7. 对讲模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/talk/start/:device_id/:channel_id | 开始对讲 | ✅ 完整实现 |
| GET /api/talk/stop/:device_id/:channel_id | 停止对讲 | ✅ 完整实现 |
| GET /api/talk/invite/:device_id/:channel_id | 邀请对讲 | ✅ 完整实现 |
| POST /api/talk/ack | 对讲确认 | ✅ 完整实现 |
| POST /api/talk/bye | 对讲结束 | ✅ 完整实现 |
| GET /api/talk/status/:device_id/:channel_id | 对讲状态 | ✅ 完整实现 |
| GET /api/talk/list | 对讲列表 | ✅ 完整实现 |

### 8. 推流模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/push/list | 推流列表 | ✅ 完整实现 |
| POST /api/push/add | 添加推流 | ✅ 完整实现 |
| POST /api/push/update | 更新推流 | ✅ 完整实现 |
| GET /api/push/start | 开始推流 | ✅ 完整实现 |
| POST /api/push/remove | 删除推流 | ✅ 完整实现 |
| POST /api/push/upload | 上传推流 | ✅ 完整实现 |
| DELETE /api/push/batchRemove | 批量删除 | ✅ 完整实现 |
| POST /api/push/save_to_gb | 保存到国标 | ✅ 完整实现 |
| DELETE /api/push/remove_form_gb | 从国标移除 | ✅ 完整实现 |

### 9. 拉流代理模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/proxy/list | 代理列表 | ✅ 完整实现 |
| GET /api/proxy/ffmpeg_cmd/list | FFmpeg命令模板 | ✅ 完整实现 |
| POST /api/proxy/add | 添加代理 | ✅ 完整实现 |
| POST /api/proxy/update | 更新代理 | ✅ 完整实现 |
| POST /api/proxy/save | 保存代理 | ✅ 完整实现 |
| GET /api/proxy/start | 开始代理 | ✅ 完整实现 |
| GET /api/proxy/stop | 停止代理 | ✅ 完整实现 |
| DELETE /api/proxy/delete | 删除代理 | ✅ 完整实现 |

### 10. 级联平台模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/platform/query | 平台查询 | ✅ 完整实现 |
| GET /api/platform/server_config | 服务器配置 | ✅ 完整实现 |
| GET /api/platform/channel/list | 通道列表 | ✅ 完整实现 |
| POST /api/platform/add | 添加平台 | ✅ 完整实现 |
| POST /api/platform/update | 更新平台 | ✅ 完整实现 |
| DELETE /api/platform/delete | 删除平台 | ✅ 完整实现 |
| GET /api/platform/exit/:device_gb_id | 退出平台 | ✅ 完整实现 |
| POST /api/platform/catalog/add | 添加目录 | ✅ 完整实现 |
| POST /api/platform/catalog/edit | 编辑目录 | ✅ 完整实现 |

### 11. 区域管理模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/region/tree/list | 区域树列表 | ✅ 完整实现 |
| DELETE /api/region/delete | 删除区域 | ✅ 完整实现 |
| GET /api/region/description | 区域详情 | ✅ 完整实现 |
| GET /api/region/addByCivilCode | 按行政区划添加 | ✅ 完整实现 |
| POST /api/region/add | 添加区域 | ✅ 完整实现 |
| POST /api/region/update | 更新区域 | ✅ 完整实现 |
| GET /api/region/path | 区域路径 | ✅ 完整实现 |
| GET /api/region/tree/query | 区域树查询 | ✅ 完整实现 |

### 12. 分组管理模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/group/tree/list | 分组树列表 | ✅ 完整实现 |
| POST /api/group/add | 添加分组 | ✅ 完整实现 |
| POST /api/group/update | 更新分组 | ✅ 完整实现 |
| DELETE /api/group/delete | 删除分组 | ✅ 完整实现 |
| GET /api/group/path | 分组路径 | ✅ 完整实现 |
| GET /api/group/tree/query | 分组树查询 | ✅ 完整实现 |

### 13. 录像计划模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/record/plan/get | 获取计划 | ✅ 完整实现 |
| POST /api/record/plan/add | 添加计划 | ✅ 完整实现 |
| POST /api/record/plan/update | 更新计划 | ✅ 完整实现 |
| GET /api/record/plan/query | 查询计划 | ✅ 完整实现 |
| DELETE /api/record/plan/delete | 删除计划 | ✅ 完整实现 |
| GET /api/record/plan/channel/list | 计划通道列表 | ✅ 完整实现 |
| POST /api/record/plan/link | 关联计划 | ✅ 完整实现 |

### 14. 报警管理模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/alarm/list | 报警列表 | ✅ 完整实现 |
| GET /api/alarm/detail/:id | 报警详情 | ✅ 完整实现 |
| POST /api/alarm/handle | 处理报警 | ✅ 完整实现 |
| DELETE /api/alarm/delete/:id | 删除报警 | ✅ 完整实现 |
| SIP报警入库 | 报警持久化 | ✅ 完整实现 |
| WebSocket广播 | 报警推送 | ✅ 完整实现 |

### 15. API Key管理模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/userApiKey/userApiKeys | API Key列表 | ✅ 完整实现 |
| POST /api/userApiKey/add | 添加API Key | ✅ 完整实现 |
| POST /api/userApiKey/enable | 启用API Key | ✅ 完整实现 |
| POST /api/userApiKey/disable | 禁用API Key | ✅ 完整实现 |
| POST /api/userApiKey/reset | 重置API Key | ✅ 完整实现 |
| DELETE /api/userApiKey/delete | 删除API Key | ✅ 完整实现 |
| POST /api/userApiKey/remark | 修改备注 | ✅ 完整实现 |

### 16. 云端录像模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/cloud/record/list | 录像列表 | ✅ 完整实现 |
| GET /api/cloud/record/play/path | 播放路径 | ✅ 完整实现 |
| GET /api/cloud/record/date/list | 日期列表 | ✅ 完整实现 |
| GET /api/cloud/record/loadRecord | 加载录像 | ✅ 完整实现 |
| GET /api/cloud/record/seek | 跳转播放 | ✅ 完整实现 |
| GET /api/cloud/record/speed | 播放倍速 | ✅ 完整实现 |
| GET /api/cloud/record/task/add | 添加任务 | ✅ 完整实现 |
| GET /api/cloud/record/task/list | 任务列表 | ✅ 完整实现 |
| DELETE /api/cloud/record/delete | 删除录像 | ✅ 完整实现 |

### 17. JT1078 部标设备模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/jt1078/terminal/list | 终端列表 | ✅ 完整实现 |
| POST /api/jt1078/terminal/add | 添加终端 | ✅ 完整实现 |
| POST /api/jt1078/terminal/update | 更新终端 | ✅ 完整实现 |
| DELETE /api/jt1078/terminal/delete | 删除终端 | ✅ 完整实现 |
| GET /api/jt1078/live/start | 开始直播 | ✅ 完整实现 |
| GET /api/jt1078/live/stop | 停止直播 | ✅ 完整实现 |
| GET /api/jt1078/playback/start | 开始回放 | ✅ 完整实现 |
| GET /api/jt1078/playback/stop | 停止回放 | ✅ 完整实现 |
| GET /api/jt1078/ptz | 云台控制 | ✅ 完整实现 |
| GET /api/jt1078/record/list | 录像列表 | ✅ 完整实现 |
| ... | 更多JT1078功能 | ✅ 完整实现 |

### 18. 通用通道模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/common/channel/list | 通道列表 | ✅ 完整实现 |
| GET /api/common/channel/one | 通道详情 | ✅ 完整实现 |
| POST /api/common/channel/add | 添加通道 | ✅ 完整实现 |
| POST /api/common/channel/update | 更新通道 | ✅ 完整实现 |
| POST /api/common/channel/reset | 重置通道 | ✅ 完整实现 |
| GET /api/common/channel/play | 播放通道 | ✅ 完整实现 |
| GET /api/common/channel/play/stop | 停止播放 | ✅ 完整实现 |

### 19. 电子地图模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/server/map/config | 地图配置 | ✅ 完整实现 |
| GET /api/common/channel/map/list | 地图通道列表 | ✅ 完整实现 |
| POST /api/common/channel/map/save-level | 保存地图级别 | ✅ 完整实现 |
| POST /api/common/channel/map/reset-level | 重置地图级别 | ✅ 完整实现 |
| GET /api/common/channel/map/thin/clear | 清除抽稀 | ✅ 完整实现 |
| GET /api/common/channel/map/thin/progress | 抽稀进度 | ✅ 完整实现 |
| GET /api/common/channel/map/thin/save | 保存抽稀 | ✅ 完整实现 |
| POST /api/common/channel/map/thin/draw | 绘制抽稀 | ✅ 完整实现 |

### 20. 前端控制模块 ✅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/front-end/ptz/:device_id/:channel_id | 云台控制 | ✅ 完整实现 |
| GET /api/front-end/preset/* | 预置位控制 | ✅ 完整实现 |
| GET /api/front-end/cruise/* | 巡航控制 | ✅ 完整实现 |
| GET /api/front-end/scan/* | 扫描控制 | ✅ 完整实现 |

### 21. Redis缓存模块 ✅
| 功能 | 状态 |
|------|------|
| 设备在线状态缓存 | ✅ 完整实现 |
| 流信息缓存 | ✅ 完整实现 |
| 媒体服务器流计数 | ✅ 完整实现 |
| 录像状态缓存 | ✅ 完整实现 |

### 22. 负载均衡模块 ✅
| 功能 | 状态 |
|------|------|
| 最少连接数策略 | ✅ 完整实现 |
| Redis计数支持 | ✅ 完整实现 |
| 无Redis降级方案 | ✅ 完整实现 |

### 23. WebSocket模块 ✅
| 功能 | 状态 |
|------|------|
| 设备状态推送 | ✅ 完整实现 |
| 报警推送 | ✅ 完整实现 |
| 录像状态推送 | ✅ 完整实现 |

### 24. SIP协议模块 ✅
| 功能 | 状态 |
|------|------|
| 设备注册 | ✅ 完整实现 |
| 心跳保活 | ✅ 完整实现 |
| 目录查询 | ✅ 完整实现 |
| 实时视频 | ✅ 完整实现 |
| 录像回放 | ✅ 完整实现 |
| 云台控制 | ✅ 完整实现 |
| 报警订阅 | ✅ 完整实现 |
| 目录订阅自动续期 | ✅ 完整实现 |

---

## 二、未完全实现的功能（Stub/占位实现）

### 1. 移动位置订阅
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/device/query/subscribe/mobile-position | 移动位置订阅 | ⚠️ Stub实现 |

**问题**: 当前仅返回成功，未发送 SIP SUBSCRIBE，未持久化订阅周期

### 2. 设备配置查询
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/device/config/query/:device_id/BasicParam | 基本参数查询 | ⚠️ Stub实现 |

**问题**: 当前返回空对象，未发送 SIP 查询命令

### 3. 日志管理
| 端点 | 功能 | 状态 |
|------|------|------|
| GET /api/log/list | 日志列表 | ⚠️ 简单实现 |
| GET /api/log/file/:file_name | 日志下载 | ✅ 完整实现 |

**问题**: 日志列表仅简单查询，缺少分页和过滤功能

---

## 三、未添加的功能

### 1. SIP服务端功能（被动模式）
| 功能 | 说明 |
|------|------|
| 作为下级平台注册 | 当前仅支持作为上级平台接收下级注册 |
| 被动目录查询 | 作为下级响应目录查询 |

### 2. 高级录像管理
| 功能 | 说明 |
|------|------|
| 录像片段合并 | 多个录像片段合并为一个 |
| 录像转码 | 录像格式转换 |
| 录像截图 | 从录像中提取截图 |

### 3. 智能分析
| 功能 | 说明 |
|------|------|
| 人脸识别 | 视频人脸检测与识别 |
| 车辆识别 | 车辆检测与车牌识别 |
| 行为分析 | 异常行为检测 |

### 4. 系统管理
| 功能 | 说明 |
|------|------|
| 系统日志详细记录 | 操作日志、访问日志 |
| 在线用户管理 | 查看在线用户、强制下线 |
| 系统监控 | CPU、内存、磁盘监控 |

### 5. 其他功能
| 功能 | 说明 |
|------|------|
| 级联平台状态监控 | 平台连接状态、通道同步状态 |
| 设备分组批量操作 | 批量移动、删除 |
| 录像计划模板 | 预定义录像计划模板 |

---

## 四、功能覆盖率统计

| 模块 | 已实现 | 未完全实现 | 未添加 | 覆盖率 |
|------|--------|------------|--------|--------|
| 用户管理 | 9 | 0 | 0 | 100% |
| 设备管理 | 9 | 0 | 0 | 100% |
| 设备控制 | 5 | 0 | 0 | 100% |
| 媒体服务 | 6 | 0 | 0 | 100% |
| 实时视频 | 4 | 0 | 0 | 100% |
| 回放模块 | 8 | 0 | 0 | 100% |
| 对讲模块 | 7 | 0 | 0 | 100% |
| 推流模块 | 9 | 0 | 0 | 100% |
| 拉流代理 | 8 | 0 | 0 | 100% |
| 级联平台 | 9 | 0 | 0 | 100% |
| 区域管理 | 8 | 0 | 0 | 100% |
| 分组管理 | 6 | 0 | 0 | 100% |
| 录像计划 | 7 | 0 | 0 | 100% |
| 报警管理 | 6 | 0 | 0 | 100% |
| API Key | 7 | 0 | 0 | 100% |
| 云端录像 | 9 | 0 | 0 | 100% |
| JT1078 | 20+ | 0 | 0 | 100% |
| 通用通道 | 15+ | 0 | 0 | 100% |
| 电子地图 | 8 | 0 | 0 | 100% |
| 前端控制 | 15+ | 0 | 0 | 100% |
| Redis缓存 | 4 | 0 | 0 | 100% |
| 负载均衡 | 3 | 0 | 0 | 100% |
| WebSocket | 3 | 0 | 0 | 100% |
| SIP协议 | 8 | 0 | 0 | 100% |
| 移动位置 | 0 | 1 | 0 | 0% |
| 设备配置 | 0 | 1 | 0 | 0% |
| 日志管理 | 1 | 1 | 0 | 50% |
| SIP服务端 | 0 | 0 | 2 | 0% |
| 高级录像 | 0 | 0 | 3 | 0% |
| 智能分析 | 0 | 0 | 3 | 0% |
| 系统管理 | 0 | 0 | 3 | 0% |

**总体覆盖率**: 约 **95%** 的核心功能已完全实现

---

## Impact
- 本文档为后续开发提供清晰的功能对比参考
- 明确了需要完善的功能点（移动位置订阅、设备配置查询）
- 明确了可选的扩展功能（智能分析、高级录像管理等）
