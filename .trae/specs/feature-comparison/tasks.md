# Tasks

## 已完成的功能实现

- [x] Redis 缓存集成
  - [x] 添加 redis 依赖到 Cargo.toml
  - [x] 创建 src/cache.rs 实现缓存操作
  - [x] 在 lib.rs 中初始化 Redis 连接
  - [x] 实现设备在线状态缓存
  - [x] 实现流信息缓存
  - [x] 实现媒体服务器流计数
  - [x] 实现录像状态缓存

- [x] 多节点负载均衡（最少连接数策略）
  - [x] 实现 select_least_loaded 函数
  - [x] Redis 计数支持
  - [x] 无 Redis 降级方案
  - [x] zlm/client.rs 添加 get_active_stream_count

- [x] 报警订阅与持久化
  - [x] 创建 db/alarm.rs 实现报警 CRUD
  - [x] 修改 sip/server.rs handle_alarm 入库
  - [x] WebSocket 广播报警事件

- [x] 目录订阅自动续期
  - [x] 添加 get_devices_for_catalog_renewal 函数
  - [x] 实现后台续期任务（每60秒检查）
  - [x] 添加 send_subscribe_internal 方法

- [x] 电子地图功能补全
  - [x] 实现 update_map_level 函数
  - [x] 实现 reset_map_level 函数
  - [x] map_config 返回真实配置
  - [x] map_save_level/map_reset_level 真实逻辑

- [x] 推流上传端点
  - [x] 添加 push_upload 函数
  - [x] 添加 /api/push/upload 路由

- [x] 设备录像控制完善
  - [x] 添加 Redis 状态跟踪
  - [x] WebSocket 广播录像状态

## 待完善的功能

- [x] 移动位置订阅完善
  - [x] 实现 SIP SUBSCRIBE 发送
  - [x] 持久化订阅周期
  - [x] 添加自动续期机制

- [x] 设备配置查询完善
  - [x] 实现 SIP ConfigQuery 命令发送
  - [x] 解析并返回配置参数

- [x] 日志管理完善
  - [x] 添加分页功能
  - [x] 添加时间范围过滤
  - [x] 添加日志类型过滤

## 可选扩展功能

- [ ] SIP 服务端功能（被动模式）
- [ ] 高级录像管理
- [ ] 智能分析集成
- [ ] 系统监控功能

# Task Dependencies

无依赖关系，各功能模块独立实现。
