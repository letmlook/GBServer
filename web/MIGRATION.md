# GBServer Web-V3 迁移清单

> 源分支：`feat-refactor-frontend-OcNoxC`（已含 Vue 2 + Element UI 的设计系统重构）
> 目标：`web-v3/`（Vue 3 + Element Plus + Vite + TypeScript）
> 状态：**Phase 1 完成**，下表为剩余 Phase 2+ 待办。

## Phase 1 ✅（本次 PR）

- Vite + TS + Vue 3.5 + Pinia + Vue Router 4 脚手架
- 设计系统：`web/src/styles/*` → `web-v3/src/styles/*`（保持设计 Token 同步）
- 工具：request / auth / validate / settings
- 类型：WvpResult、RouteMeta、TagView
- Store：user / app / settings / tagsView
- 路由 + permission 守卫
- 完整布局壳（Layout + 6 个子组件）
- 登录页 + 控制台页 + 404
- SVG 图标雪碧图

## Phase 2 · 路由扩展 + 基础业务页

| 路径 | 旧文件 | 优先级 | 备注 |
|------|--------|--------|------|
| `/channel` | views/channel | P0 | 通道列表（gb-page + 表格） |
| `/live` | views/live | P0 | 直播列表 + 视频墙（VideoCell 重用） |
| `/playback` | views/playback | P0 | 录像回放（时间轴） |
| `/map` | views/map | P1 | 自绘 SVG 地图（已具备，搬过来） |
| `/mediaServer` | views/mediaServer | P0 | 媒体节点列表 |
| `/recordPlan` | views/recordPlan | P1 | 录像计划 |

## Phase 3 · 平台 / 资源 / 组织

| 路径 | 旧文件 | 优先级 | 备注 |
|------|--------|--------|------|
| `/platform` | views/platform | P0 | 上级 / 下级平台 |
| `/streamProxy` | views/streamProxy | P1 | 拉流代理 |
| `/user` | views/user | P0 | 用户管理（含标签页） |
| `/jtDevice` | views/jtDevice | P2 | JT/T 1078 车载终端 |

## Phase 4 · 运维

| 路径 | 旧文件 | 优先级 | 备注 |
|------|--------|--------|------|
| `/operations/realLog` | views/operations/realLog | P0 | 实时日志 |
| `/operations/historyLog` | views/operations/historyLog | P0 | 历史日志 |
| `/operations/systemInfo` | views/operations/systemInfo | P1 | 系统信息 |

## Phase 5 · 第三方库替代

| 旧依赖 | 替代方案 | 原因 |
|--------|----------|------|
| `vue-clipboard2` | `vue-clipboard-next` 或 `navigator.clipboard` | 旧库不支持 Vue 3 |
| `v-charts` | `@vuescroll/vue-echarts` 或 `echarts` 直用 | 旧库已停更 |
| `vue-ztree-2.0` / `vue-contextmenujs` | `element-plus` `el-tree` + 自写右键 | 旧库无 Vue 3 维护 |
| `jessibuca` / `h265web.js` | 待评估：维持 jessibuca（player 独立于框架） | 播放器与框架解耦 |
| `screenfull` | `screenfull`（已支持） | 无变更 |
| `js-md5` | 保留（已加） | 无变更 |
| `moment` | `dayjs`（已加） | 体积更小 |
| `vuex` | `pinia`（已替换） | Vue 3 推荐 |

## Phase 6 · 组件库

- 完成 `Pagination`、`Upload`、`RichText`、`PanThumb`、`BackToTop`、`ErrorLog` 等通用组件
- 沉淀 `<gb-table>` 通用 CRUD 表格封装
- `<gb-search-form>` 通用搜索表单

## Phase 7 · API 类型化

- 把 `api/*.js` 全量迁移为 `.ts`
- 用 `WvpResult<T>` 包装返回值
- 把常用 VO 提取到 `src/types/model.ts`（Device / Channel / User / RecordPlan …）

## 验证

```bash
cd web-v3
npm install
npm run dev          # 看到登录页 → 输入 admin/admin → 进入控制台
npm run build        # 验证 vue-tsc 类型检查通过
```

## 风险与决策

- **并行运行**：`web/` 与 `web-v3/` 同时存在；后端不感知前端。新功能优先放 `web-v3/`。
- **完成度评估**：当 `web-v3/` 覆盖 ≥ 90% 业务路径后，合并 `web/` 默认路由指向 `web-v3/dist`。
- **回滚**：保留 `web/`，任何阶段都可以回退。
