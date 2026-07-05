# GBServer Web (Vue 3 + Element Plus + Vite + TS)

> 与 `web/`（Vue 2 + Element UI + Vue CLI）并行的全新前端。
> 共享 Rust 后端 API，但使用现代栈重新搭建。

## 已迁移（Phase 1）

- ✅ Vite + TS + Vue 3.5 + Pinia + Vue Router 4
- ✅ 设计 Token、工具类、Element Plus 主题、响应式断点
- ✅ 工具层：`request`、`auth`、`validate`、类型化 `WvpResult`
- ✅ 公共组件：`SvgIcon` / `EmptyState` / `StatCard` / `VideoCell`
- ✅ Pinia store：`user` / `app` / `settings` / `tagsView`
- ✅ 路由：Vue Router 4 + lazy import + 类型化 `RouteMeta`
- ✅ 完整布局壳：`Layout` / `Navbar` / `Sidebar` / `Logo` / `TagsView` / `AppMain`
- ✅ 登录页（`/login`）
- ✅ 控制台页（`/dashboard`）
- ✅ 404 兜底

## 待迁移（Phase 2+）

剩余业务页面、第三方库替代、Pinia 扩展 store、组件库等。详见 [MIGRATION.md](./MIGRATION.md)。

## 开发

```bash
cd web-v3
npm install
npm run dev          # 启动 :9528
npm run build        # 类型检查 + 生产构建
npm run lint
```

`/dev-api` 自动代理到 `VITE_PROXY_TARGET`（默认 `http://127.0.0.1:18080`），可在 `.env.development` 修改。
