# GBServer Web

GBServer 管理后台前端，Vue 3 + Element Plus + Vite + TypeScript。
由原 Vue 2 + Element UI + Webpack 实现迁移而来（迁移过程记录见 git 历史）。

> 🔒 **代码功能已冻结（2026-09-19）** —— 本目录不再新增功能；`web-legacy-vue2/`（原 Vue 2 前端）
> 已于同日从仓库删除，仅存于 git 历史。

## 状态

| Phase | 内容 | 状态 |
|-------|------|------|
| Phase 1 | 脚手架 + 路由壳 + 登录 + 控制台 | ✅ 完成 |
| Phase 2 | 监控中心 + 资源管理业务页 | ✅ 完成 |
| Phase 3 | 平台 + 用户 + 报警 + JT1078 | ✅ 完成 |
| Phase 4 | 运维（实时日志/历史日志/系统信息） | ✅ 完成 |
| Phase 5 | 第三方库替代（js-md5, dayjs, screenfull） | ✅ 完成 |
| Phase 6 | 通用组件（Pagination / GbTable / GbSearchForm / BackToTop / Upload …） | ✅ 完成 |
| Phase 7 | API 全量类型化（17 个 API 模块 + VO 模型） | ✅ 完成 |

**规模（2026-09-19 实测）**：18 个业务视图目录（19 条业务路由）、17 个类型化 API 模块、
13 个通用组件、42 个 SVG 图标、约 15,500 行 `.ts`/`.vue`。

## 运行

```bash
cd web
npm install
npm run dev          # 开发：http://localhost:9528（/dev-api 反代到后端 :18080）
npm run type-check   # TypeScript 类型检查（vue-tsc --noEmit）
npm run build        # 类型检查 + 生产构建到 web/dist
npm run build:no-check  # 跳过类型检查直接构建
npm run preview      # 预览构建产物（:9529）
npm run lint         # ESLint
```

生产部署不跑独立前端服务：后端通过 `static_dir` 直接托管 `web/dist`（见根目录
[`docs/DEPLOYMENT_GUIDE.md`](../docs/DEPLOYMENT_GUIDE.md)）。构建后需重启后端才会加载新产物。

默认账号 `admin` / `admin`。

## 路由（19 条业务路由）

| 路由 | 标题 | 文件 |
|------|------|------|
| `/dashboard` | 控制台 | `src/views/dashboard/` |
| `/device` | 国标设备 | `src/views/device/` |
| `/channel` | 通道列表 | `src/views/channel/` |
| `/live` | 实时直播（支持 WebRTC / FLV / HLS） | `src/views/live/` |
| `/playback` | 录像回放 | `src/views/playback/` |
| `/cloudRecord` | 云端录像 | `src/views/cloudRecord/` |
| `/mediaServer` | 媒体节点 | `src/views/mediaServer/` |
| `/recordPlan` | 录像计划 | `src/views/recordPlan/` |
| `/platform` | 上级平台 | `src/views/platform/` |
| `/streamProxy` | 拉流代理 | `src/views/streamProxy/` |
| `/streamPush` | 推流列表 | `src/views/streamPush/` |
| `/region` | 行政区划 / 业务分组 | `src/views/region/` |
| `/map` | 电子地图（Leaflet + 高德栅格瓦片） | `src/views/map/` |
| `/alarm` | 报警管理 | `src/views/alarm/` |
| `/user` | 用户管理 | `src/views/user/` |
| `/jtDevice` | JT1078 终端 | `src/views/jtDevice/` |
| `/operations/realLog` | 实时日志 | `src/views/operations/` |
| `/operations/historyLog` | 历史日志 | `src/views/operations/` |
| `/operations/systemInfo` | 系统信息 | `src/views/operations/` |

另有 `/login`、`/404`、`/redirect/:path(.*)*` 等非业务路由。
路由使用 **hash 模式**（`createWebHashHistory`）—— e2e 与手测都必须写 `/#/live` 这类 URL，
写成 `/live` 会被解析成 `/` 并重定向到 `#/dashboard`。

## 项目结构

```
src/
├── api/                # 类型化 API 客户端（17 个模块）
│   ├── alarm.ts channel.ts cloudRecord.ts device.ts jtDevice.ts live.ts
│   ├── log.ts mediaServer.ts platform.ts playback.ts recordPlan.ts
│   ├── region.ts streamProxy.ts streamPush.ts syCamera.ts talk.ts user.ts
├── components/         # 通用组件（13 个）
│   ├── BackToTop/ ChannelPlayDialog/ EmptyState/ GbSearchForm/ GbTable/
│   ├── Pagination/ PlatformInfo/ SnapPreview/ StatCard/ SvgIcon/
│   ├── TalkPanel/ Upload/ VideoCell/
├── composables/        # Vue 组合式函数（useTagsViewSync）
├── icons/              # SVG 图标（42 个）
├── layout/             # 布局壳（Navbar/Sidebar/TagsView）
├── router/             # Vue Router 4（hash 模式）
├── store/              # Pinia store
├── styles/             # 全局样式 + Element Plus 主题
├── types/              # TypeScript 类型定义
│   ├── api.ts          # ApiResult<T> / PageQuery / PageResult
│   └── model.ts        # DeviceVO / ChannelVO / PlatformVO 等 VO
├── utils/              # request / auth / validate / get-page-title
├── views/              # 业务页面（18 个目录 + 404.vue / redirect.vue）
└── App.vue / main.ts / permission.ts
```

## API 类型化约定

所有 API 返回 `ApiResult<T>`：

```ts
interface ApiResult<T> {
  code: number      // 0 = 成功，其他为业务错误
  msg: string
  data: T
}
```

分页接口返回 `{ total: number; list: T[] }`，统一封装在 `src/types/api.ts::PageResult<T>`。
请求统一走 `src/utils/request.ts`（注入 `access-token` 头，`code !== 0` 视为失败）。

## 已知限制

- **实时日志**：当前是 5s 轮询历史 API 兜底，不是真实推送；生产需要后端提供
  `/api/log/stream` WebSocket 推送（见 `src/views/operations/realLog.vue:93`）。
- **GB28181 流走 WebRTC 解不出帧**：入口已接（直播页 + 通道播放对话框），但国标流经
  WebRTC 仍收不到帧；普通流正常。详见根目录 [`docs/OPEN_ISSUES.md`](../docs/OPEN_ISSUES.md) B8。
- **国际化**：当前硬编码中文；如需英文版需引入 vue-i18n。

## 版本

- Vue 3.5.12
- Element Plus 2.8.6
- Vite 5.4.10
- TypeScript 5.6.3
- Pinia 2.2.6
- Leaflet（电子地图）
