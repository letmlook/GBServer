# GBServer UI 操作指南（UI_WALKTHROUGH.md）

> 配合 2026-06-20 调试使用，所有页面截图保存在 `e2e/artifacts/*.png`

## 0. 访问地址

| 入口 | URL | 说明 |
|------|-----|------|
| 前端登录页 | http://localhost:9528/ | Vue dev server |
| 前端 → 后端代理 | http://localhost:9528/dev-api/* | webpack proxy 到 :18080 |
| 后端 HTTP | http://localhost:18080/api/* | 直接调后端 |
| 后端 health | http://localhost:18080/api/health | 存活探针 |
| 后端 metrics | http://localhost:18080/metrics | Prometheus 指标 |
| ZLM HTTP API | http://localhost:18081/index/api/* | 流媒体服务（需 secret） |

默认账号 `admin` / `admin`。

## 1. 登录

1. 浏览器打开 `http://localhost:9528/`
2. 看到登录页（`artifacts/login.png`）
3. 输入 `admin` / `admin`
4. 点 **登录**
5. 自动跳转到 `/dashboard`

**后端验证**：
```bash
curl 'http://127.0.0.1:18080/api/user/login?username=admin&password=admin'
# 返回：{"code":0,"msg":"成功","data":{...,"accessToken":"eyJ..."}}
```

## 2. 控制台 (Dashboard)

URL: `/dashboard`（`artifacts/dashboard.png`）

看到的元素：
- 左侧菜单：控制台 / 分屏监控 / 通道列表 / 电子地图 / 设备接入 / 组织结构 / 录制计划 / 云端录像 / 媒体节点 / 国标级联 / 用户管理 / 运维中心
- 顶部：欢迎 admin
- 4 个统计环：设备总数/通道总数/推流总数/拉流代理总数（数值都为 0，预期）
- 多个 echarts 图表（设备使用率、上传下载带宽、推流类型分布等）
- 底部：zlmediakit-1 节点标识（绿色 legend = 在线）

**后端 API**：
- `GET /api/dashboard/overview` → 概览数据
- `GET /api/server/media_server/list` → 媒体节点列表

## 3. 设备管理

URL: `/device` → 选 "国标设备"（`artifacts/device.png`）

看到的元素：
- 搜索框（关键字）
- 在线状态下拉
- **+ 添加设备** 按钮
- **接入信息** 按钮
- 设备列表（默认显示 self 设备 GBServer / device_id=34020000002000000001）
- 操作列：刷新 / 通道 / 编辑 / 操作

**添加设备**（手工 UI 步骤）：
1. 点 **+ 添加设备**
2. 弹窗中填：
   - 设备 ID：20 位国标 ID（例 `34020000001320000001`）
   - 名称：自定义
   - 厂家 / 型号：可选
   - 传输模式：UDP/TCP
3. 点确定 → 后端 `POST /api/device/query/devices/:device_id` 创建
4. 设备会出现在列表（默认 offline）
5. 真实设备需向 GBServer SIP 5060 发起 REGISTER 才能上线

**后端 API**：
- `GET /api/device/query/devices?page=1&count=10` → 列表
- `POST /api/device/query/devices/:device_id` → 新增（stub 实现）
- `GET /api/device/query/devices/.../channels` → 通道
- `GET /api/device/query/statistics/register` → 注册统计

## 4. 媒体节点

URL: `/mediaServer`（`artifacts/mediaServer.png`）

看到的元素：
- **+ 添加节点** 按钮
- 节点卡：zlmediakit-1 / 127.0.0.1
- 状态点（圆点，灰色 = 健康检查未拉到，绿色 = 在线）
- 删除按钮（垃圾桶图标）

**添加节点**：
1. 点 **+ 添加节点**
2. 填：id / IP / HTTP 端口 / Secret
3. 确定 → 后端保存到 `gb_media_server` 表

**后端 API**：
- `GET /api/server/media_server/list` → 列表
- `POST /api/server/media_server/save` → 新增/更新
- `GET /api/server/media_server/check` → 健康检查

## 5. 用户管理

URL: `/user`（`artifacts/user.png`）

看到的元素：
- **+ 添加用户** 按钮
- 用户列表：用户名 / pushkey / 类型(admin) / 操作
- 操作：修改密码 / 修改 pushkey / 管理 ApiKey / 删除

**添加用户**：
1. 点 **+ 添加用户**
2. 填：用户名 / 密码 / pushkey（自动生成可空）
3. 确定 → `POST /api/user/add`

**后端 API**：
- `GET /api/user/users?page=1&count=10` → 列表
- `POST /api/user/add` → 新增
- `POST /api/user/changePassword` → 改密
- `DELETE /api/user/delete` → 删除

## 6. 通道列表 / 分屏监控 / 直播

URL: `/channel` / `/live`（`artifacts/channel.png` / `artifacts/live.png`）

- 通道列表：按设备组织，显示所有 SIP 通道
- 分屏监控：1/4/9/16 宫格播放（需要真实视频流）
- 直播：单画面播放

**注意**：当前没有真实摄像头注册，所以**所有通道列表为空**，**所有分屏画面是空的**。
要测试播放，需要：1) 添加设备 → 2) 设备 SIP REGISTER → 3) 设备推流到 ZLM → 4) 后端查 SSRC → 5) 播放。

## 7. 录制计划 / 云端录像

URL: `/recordPlan` / `/cloudRecord`（`artifacts/recordPlan.png` / `artifacts/cloudRecord.png`）

- 录制计划：CRUD（时间段/通道关联）
- 云端录像：列表/查询/打包下载/收藏

## 8. 国标级联

URL: `/platform`（`artifacts/platform.png`）

- 上级平台列表（GBServer 可作为下级向上级注册）
- 添加/编辑/启用

## 9. 运维中心

URL: `/operations`（`artifacts/operations.png`）

- 系统信息（CPU/内存/磁盘/网络）
- **已知问题**：`OperationsSystemInfo` 组件报 `childValue.startsWith is not a function`（后端返回非字符串字段）

## 10. 告警

URL: `/alarm`（`artifacts/alarm.png`）

- 设备告警订阅 + 告警事件列表

## 11. 公共通道

URL: `/commonChannel`（`artifacts/commonChannel.png`）

- 把多个设备/通道合并成"公共通道"（地图点位用）
- **已知问题**：渲染时 `TypeError: Cannot read properties of undefined (reading 'offsetHeight')`

## 12. 推流 / 拉流代理

URL: `/push` / `/proxy`（`artifacts/push.png` / `artifacts/proxy.png`）

- 推流：GB28181 推流列表（设备 → ZLM）
- 拉流代理：FFmpeg 命令启停的代理流

## 13. 电子地图

URL: `/map`（`artifacts/map.png`）

- OpenStreetMap 瓦片地图，显示所有通道点位
- 需要 `config/application.toml` 的 `[map] enabled = true`（当前 false）

## 关键路径速查

| 任务 | UI 入口 | API 路径 |
|------|---------|----------|
| 登录 | `/` → admin/admin | `GET /api/user/login` |
| 看设备 | `/device` | `GET /api/device/query/devices` |
| 看通道 | `/device` → 设备 → 通道 | `GET /api/device/query/devices/:id/channels` |
| 加设备 | `/device` → +添加设备 | `POST /api/device/query/devices` |
| 看媒体节点 | `/mediaServer` | `GET /api/server/media_server/list` |
| 加节点 | `/mediaServer` → +添加节点 | `POST /api/server/media_server/save` |
| 看用户 | `/user` | `GET /api/user/users` |
| 加用户 | `/user` → +添加用户 | `POST /api/user/add` |
| 改密 | `/user` → 修改密码 | `POST /api/user/changePassword` |
| 录制计划 | `/recordPlan` | `GET /api/recordPlan/list` (待确认) |
| 级联平台 | `/platform` | `GET /api/platform/list` (待确认) |

## 14. 自动化 UI 测试

项目自带 `e2e/tests/smoke.spec.ts` 覆盖 15 个核心页面的渲染+截图。

```bash
cd e2e
npx playwright install chromium  # 首次需装
npx playwright test              # 跑全部
# 截图在 e2e/artifacts/*.png
```

测试会用真实 admin/admin 登录 + 跳转到每个路由 + 截图 + 收集 console.error。
