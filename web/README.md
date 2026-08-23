# GBServer Web V3

Vue 3 + Element Plus + Vite + TypeScript 重写的 GBServer 管理后台。
替代原 Vue 2 + Element UI + Webpack 的 [web/](../web/) 实现。

## 状态

| Phase | 内容 | 状态 |
|-------|------|------|
| Phase 1 | 脚手架 + 路由壳 + 登录 + 控制台 | ✅ 完成 |
| Phase 2 | 监控中心 + 资源管理业务页 | ✅ 完成 |
| Phase 3 | 平台 + 用户 + 报警 + JT1078 | ✅ 完成 |
| Phase 4 | 运维（实时日志/历史日志/系统信息） | ✅ 完成 |
| Phase 5 | 第三方库替代（js-md5, dayjs, screenfull） | ✅ 完成 |
| Phase 6 | 通用组件（Pagination / GbTable / GbSearchForm / BackToTop / Upload） | ✅ 完成 |
| Phase 7 | API 全量类型化（14 个 API 模块 + 14 个 VO 模型） | ✅ 完成 |

**完成度**：所有 17 个业务路由可用 + 控制台动态数据 + 通用组件封装 + 类型检查 0 错误 + Vite 构建 4.4s。

## 运行

```bash
cd web-v3
npm install
npm run dev          # 开发：http://localhost:9529
npm run type-check   # TypeScript 类型检查（vue-tsc --noEmit）
npm run build        # 生产构建到 dist/
npm run preview      # 预览构建产物
```

默认账号 `admin` / `admin`。

## 路由（17 条）

| 路由 | 标题 | 文件 |
|------|------|------|
| `/dashboard` | 控制台 | `src/views/dashboard/` |
| `/device` | 国标设备 | `src/views/device/` |
| `/channel` | 通道列表 | `src/views/channel/` |
| `/live` | 实时直播 | `src/views/live/` |
| `/playback` | 录像回放 | `src/views/playback/` |
| `/cloudRecord` | 云端录像 | `src/views/cloudRecord/` |
| `/mediaServer` | 媒体节点 | `src/views/mediaServer/` |
| `/recordPlan` | 录像计划 | `src/views/recordPlan/` |
| `/platform` | 上级平台 | `src/views/platform/` |
| `/streamProxy` | 拉流代理 | `src/views/streamProxy/` |
| `/streamPush` | 推流列表 | `src/views/streamPush/` |
| `/map` | 电子地图 | `src/views/map/` |
| `/alarm` | 报警管理 | `src/views/alarm/` |
| `/user` | 用户管理 | `src/views/user/` |
| `/jtDevice` | JT1078 终端 | `src/views/jtDevice/` |
| `/operations/realLog` | 实时日志 | `src/views/operations/` |
| `/operations/historyLog` | 历史日志 | `src/views/operations/` |
| `/operations/systemInfo` | 系统信息 | `src/views/operations/` |

## 项目结构

```
src/
├── api/                # 类型化 API 客户端（14 个模块）
│   ├── alarm.ts cloudRecord.ts device.ts jtDevice.ts live.ts
│   ├── log.ts mediaServer.ts platform.ts playback.ts
│   ├── recordPlan.ts region.ts streamProxy.ts streamPush.ts
│   └── user.ts
├── components/         # 通用组件
│   ├── BackToTop/ EmptyState/ GbSearchForm/ GbTable/
│   ├── Pagination/ StatCard/ SvgIcon/ Upload/ VideoCell/
├── composables/        # Vue 组合式函数
├── icons/              # SVG 图标（41 个）
├── layout/             # 布局壳（Navbar/Sidebar/TagsView）
├── router/             # Vue Router 4
├── store/              # Pinia store
├── styles/             # 全局样式 + Element Plus 主题
├── types/              # TypeScript 类型定义
│   ├── api.ts          # WvpResult<T> / PageQuery / PageResult
│   └── model.ts        # DeviceVO / ChannelVO / PlatformVO 等 14 个 VO
├── utils/              # request / auth / get-page-title
├── views/              # 业务页面（17 个）
└── App.vue / main.ts / permission.ts
```

## API 类型化约定

所有 API 返回 `WvpResult<T>`：

```ts
interface WvpResult<T> {
  code: number      // 0 = 成功，其他为业务错误
  msg: string
  data: T
}
```

分页接口返回 `{ total: number; list: T[] }`，统一封装在 `src/types/api.ts::PageResult<T>`。

## 与 web/ 并行策略

`web/` 与 `web-v3/` 同时存在，后端不感知前端。新功能优先在 `web-v3/` 实现。当 `web-v3/` 覆盖 ≥ 90% 业务路径后，将 `web/dist` 切换为 `web-v3/dist` 并下线 `web/`。

## 风险

- **JT1078 协议操作层**：live/record/snap 等端点仍为"已受理"响应（需在线终端 session）
- **实时日志**：当前用历史列表 + 1.5s 推送模拟；真实生产需要 WebSocket 推送
- **电子地图**：用 SVG 占位渲染点位；待接入真实地图引擎（高德/天地图）
- **CSS 主题**：与 web/ 视觉差异需要设计评审后微调
- **国际化**：当前硬编码中文；如需英文版需引入 vue-i18n

## 版本

- Vue 3.5.12
- Element Plus 2.8.6
- Vite 5.4.10
- TypeScript 5.6.3
- Pinia 2.2.6
