# GBServer 生产可用目标对齐与差距分析

> 生成时间: 2026-05-31  
> 状态: 编译已通过，开始真实可用评估

## 一、当前状态总结

### 1.1 编译状态
- ✅ 所有新增模块编译通过（88 warnings，需后续清理）
- ✅ 提交记录: `0082e51 fix: resolve all compilation errors from new modules`

### 1.2 Parity Audit 基线（2026-05-30）

| 维度 | 官方 WVP | GBServer | 差距 |
|------|----------|----------|------|
| Java Controller Routes | 330 | 284 | -46 |
| Frontend API Calls | 250 | 244 | -6 |
| Frontend Pages | 24 | 24 | 0 |

## 二、真实可用生产差距分析

### 2.1 API 路由缺失（105 个，需按优先级实现）

#### 高优先级（核心功能）
| API | 来源 | 说明 |
|-----|------|------|
| `GET /api/play/ssrc` | PlayController | 获取 SSRC 用于直播 |
| `GET /api/play/snap` | PlayController | 获取快照 |
| `POST /api/play/convertStop/{param}` | PlayController | 停止转码 |
| `GET /api/media/getPlayUrl` | MediaController | 获取播放地址 |
| `GET /api/media/stream_info_by_app_and_stream` | MediaController | 流信息查询 |
| `GET /api/platform/info/{param}` | PlatformController | 平台信息 |

#### 中优先级（JT1078 完整功能）
| API | 来源 | 说明 |
|-----|------|------|
| `/api/jt1078/area/*` (12 个) | JT1078Controller | 区域管理 CRUD |
| `/api/jt1078/record/*` | JT1078Controller | 录像控制 |
| `/api/jt1078/route/*` | JT1078Controller | 路由管理 |
| `/api/jt1078/live/*` | JT1078Controller | 实时流控制 |
| `/api/jt1078/media/*` | JT1078Controller | 媒体上传 |

#### 低优先级（运维/辅助）
| API | 来源 | 说明 |
|-----|------|------|
| `/api/cloud/record/*` (6 个) | CloudRecordController | 云录像管理 |
| `/api/ps/*` (5 个) | PsController | PS 协议 |
| `/api/rtp/*` (4 个) | RtpController | RTP 管理 |
| `/api/server/*` (3 个) | ServerController | 服务器管理 |
| `/api/region/*` (3 个) | RegionController | 区域管理 |

### 2.2 功能模块完整性评估

#### SIP 协议栈 ✅ 基本完成
- INVITE 会话管理 ✅
- REGISTER/Keepalive ✅
- Catalog 订阅 ✅
- PTZ 控制 ✅
- BYE/ACK ✅
- **缺失**: MESSAGE 响应路由（PendingRequest 刚完成，集成未完成）

#### 视频流处理 ⚠️ 部分完成
- 实时播放 ✅（需完善 ZLM Hook 集成）
- 回放控制 ✅（PlaybackInviteSession）
- 下载 ✅（框架已有）
- 对讲/广播 ✅
- **缺失**: WebRTC 支持

#### ZLM 集成 ⚠️ 部分完成
- Hook 接收 ✅（11 种 Hook 类型）
- 流状态同步 ✅
- 媒体节点管理 ✅
- **缺失**: 自动拉流重连细节完善

#### 平台级联 ⚠️ 框架完成，集成未完成
- CascadeSession ✅
- CascadeForwarder ✅
- SendRtp 管理 ✅
- **缺失**: 与主 SIP Server 集成

#### JT1078 ⚠️ 框架完成，功能待实现
- 协议编解码 ✅
- 会话管理 ✅
- **缺失**: 命令关联、媒体会话、区域管理 API

### 2.3 前端 API 缺失（6 个）

| API | 说明 |
|-----|------|
| `DELETE /api/alarm/clear` | 清除报警 |
| `DELETE /api/alarm/delete` | 删除报警 |
| `GET /api/device/query/statistics/keepalive` | 保活统计 |
| `GET /api/device/query/statistics/register` | 注册统计 |
| `GET /api/device/query/subscribe/alarm` | 报警订阅 |

## 三、完整实施计划

### Phase 1: 核心功能补全（生产可用最低要求）

#### 1.1 MESSAGE 响应路由集成
**目标**: 完成设备查询的完整链路
- [ ] 在 `SipServer` 中初始化 `PendingRequestManager`
- [ ] 在 `handle_message()` 中集成 `ResponseRouter`
- [ ] 实现 DeviceInfo/DeviceStatus/Config/RecordInfo 查询
- [ ] 实现 Catalog 多包聚合
- [ ] 添加超时清理后台任务

#### 1.2 视频流核心功能
**目标**: 完整播放/回放/下载链路
- [ ] 完善 `send_play_invite_and_wait()` - ZLM Hook 等待媒体到达
- [ ] 实现 `get_play_url()` - 返回 rtsp/webrtc 地址
- [ ] 实现 `snap()` - 获取通道快照
- [ ] 实现 `get_ssrc()` - 获取 SSRC 信息
- [ ] 完善 `send_playback_invite()` - 回放控制
- [ ] 实现 `send_download_invite()` - 下载

#### 1.3 云录像 API
**目标**: 支持录像管理
- [ ] `GET /api/cloud/record/collect/add` - 收藏录像
- [ ] `GET /api/cloud/record/collect/delete` - 取消收藏
- [ ] `GET /api/cloud/record/list-url` - 获取录像 URL
- [ ] `GET /api/cloud/record/zip` - 打包下载
- [ ] `GET /api/cloud/record/download/zip` - 下载打包

### Phase 2: 平台级联集成

#### 2.1 上级平台注册
- [ ] 实现 `CascadeService::register()` - SIP REGISTER 到上级
- [ ] 实现 keepalive 维护
- [ ] 实现注销处理

#### 2.2 目录同步
- [ ] 向上级 NOTIFY 已共享通道
- [ ] 处理上级查询请求

#### 2.3 位置/告警上报
- [ ] MobilePosition 转发
- [ ] Alarm 转发
- [ ] SendRtp 会话管理

### Phase 3: JT1078 完整功能

#### 3.1 区域管理 API
- [ ] Circle/Polygon/Rectangle CRUD
- [ ] 区域绑定到终端

#### 3.2 实时流控制
- [ ] `GET /api/jt1078/live/pause` - 暂停
- [ ] `GET /api/jt1078/live/continue` - 恢复
- [ ] `GET /api/jt1078/live/switch` - 切换码流

#### 3.3 录像控制
- [ ] `GET /api/jt1078/record/start` - 开始录像
- [ ] `GET /api/jt1078/record/stop` - 停止录像
- [ ] `GET /api/jt1078/playback/download` - 下载回放

### Phase 4: 运维功能

#### 4.1 服务器管理
- [ ] `GET /api/server/config` - 获取配置
- [ ] `GET /api/server/version` - 获取版本
- [ ] `GET /api/server/shutdown` - 关机（可选）

#### 4.2 RTP 管理
- [ ] `GET /api/rtp/receive/open` - 开启 RTP 接收
- [ ] `GET /api/rtp/receive/close` - 关闭 RTP 接收
- [ ] `GET /api/rtp/send/start` - 开始发送
- [ ] `GET /api/rtp/send/stop` - 停止发送

#### 4.3 统计 API
- [ ] `GET /api/device/query/statistics/keepalive` - 保活统计
- [ ] `GET /api/device/query/statistics/register` - 注册统计

### Phase 5: 前端补充

#### 5.1 报警功能
- [ ] `/alarm` 页面
- [ ] 报警列表/清除/删除 API

#### 5.2 分享功能
- [ ] `/play/share` 页面
- [ ] 分享链接生成

## 四、技术债务清理

### 4.1 编译警告（88 个）
- [ ] 清理 unused imports
- [ ] 清理 unused variables
- [ ] 处理 ambiguous glob re-exports

### 4.2 代码质量
- [ ] 运行 `cargo clippy --all-targets --all-features`
- [ ] 运行 `cargo fmt`
- [ ] 添加单元测试覆盖

### 4.3 文档
- [ ] 更新 AGENTS.md
- [ ] 更新 README（如果有新功能）
- [ ] 补充 API 文档

## 五、实施优先级建议

### 立即执行（1-2 周）
1. ✅ MESSAGE 响应路由集成 - 核心查询功能
2. ✅ 视频流核心功能 - play/ssrc/snap API
3. ✅ 云录像 API - 用户可见功能

### 第二批（2-4 周）
4. 平台级联集成 - 企业级部署必需
5. JT1078 区域管理 - 车辆监控核心
6. 运维 API - 部署维护必需

### 第三批（4-8 周）
7. 前端页面 - 完整功能覆盖
8. 技术债务清理 - 长期维护基础

---

## 附录：当前模块清单

```
src/
├── sip/
│   ├── gb28181/
│   │   ├── invite_session.rs ✅ 会话管理增强
│   │   ├── pending_request.rs ✅ 命令响应管理
│   │   ├── media_waiter.rs ✅ ZLM Hook 等待
│   │   ├── playback_session.rs ✅ 回放会话
│   │   ├── device_commander.rs ✅ 设备命令
│   │   ├── catalog_sync.rs ✅ 目录同步
│   │   ├── subscription_lifecycle.rs ✅ 订阅生命周期
│   │   ├── cascade_service.rs ✅ 级联服务
│   │   ├── cascade_forward.rs ✅ 级联转发
│   │   └── invite_session.rs.bak (备份，可删除)
│   └── server.rs ✅ 集成新模块
├── zlm/
│   ├── client.rs ✅ 客户端增强
│   └── hook.rs ✅ Hook 处理
├── jt1078/
│   ├── command_waiter.rs ✅ 命令等待
│   └── jt_media_session.rs ✅ 媒体会话
├── db/
│   └── cloud_record.rs ✅ 云录像 DB
├── rpc.rs ✅ RPC 模块
├── state_store.rs ✅ 状态存储
└── handlers/
    └── stub.rs ✅ stub 路由
```