# GBServer Web · 前端

**[GBServer](../README.md)** 的 Web 管理控制台 —— 基于 Rust 实现的 GB/T 28181
视频平台的前端工程。

基于 **Vue 2 + Element UI** 的单页应用，对接 GBServer HTTP API（`/api/*`），
覆盖设备、通道、流、录像计划、级联平台、JT1078 车辆、用户与系统管理等日常运维场景。

> 🇬🇧 [English](./README.md)

---

## 📑 目录

- [项目简介](#-项目简介)
- [技术栈](#-技术栈)
- [环境要求](#-环境要求)
- [快速开始](#-快速开始)
- [目录结构](#-目录结构)
- [与后端的对接](#-与后端的对接)
- [构建与发布](#-构建与发布)
- [代码规范](#-代码规范)
- [测试](#-测试)
- [国际化](#-国际化)
- [常见问题](#-常见问题)
- [致谢](#-致谢)
- [许可证](#-许可证)

---

## 🔭 项目简介

- **定位** — GBServer 的 Web 控制台。前端页面按 GB28181 业务管理控制台的标准组织，
  只要后端 HTTP API 提供的接口，前端均能完整调用。
- **形态** — SPA；构建产物 `web/dist` 由后端在 `static_dir` 配置后接管静态托管。
- **鉴权** — `access-token`（JWT）请求头；登录、角色权限、动态路由。
- **可视化** — ECharts / v-charts、OpenLayers 地图、日志查看器、树形组件。

---

## 🛠️ 技术栈

| 层次 | 库 | 版本 |
|------|----|------|
| 框架 | Vue | 2.6.10 |
| UI 组件库 | Element UI | 2.15.x |
| 路由 | vue-router | 3.0.6 |
| 状态管理 | vuex | 3.1.0 |
| HTTP | axios | ^0.24.0 |
| 图表 | ECharts / v-charts | 4.9 / 1.19 |
| 地图 | OpenLayers | ^10.6.1 |
| 树组件 | vue-ztree-2.0 / @wchbrad/vue-easy-tree | — |
| 时间 | dayjs / moment | 1.11 / 2.29 |
| 构建 | @vue/cli-service | 4.4.4 |
| 测试 | Jest + @vue/test-utils | 23 / 1.0-beta |
| Lint | ESLint + eslint-plugin-vue | 6.7 / 6.2 |

> 引擎要求：Node `>=8.9`，npm `>=3.0.0`。**建议使用 Node 16 / 18 LTS**。

---

## ✅ 环境要求

| 工具 | 用途 | 安装 |
|------|------|------|
| **Node.js** ≥ 14（推荐 16 / 18 LTS） | 构建与开发 | <https://nodejs.org/> |
| **npm** ≥ 6（Node 自带） | 依赖管理 | — |
| **GBServer 后端** 监听 `:18080` | 开发态数据源 | 参见 [../README.md §快速开始](../README.md#-快速开始) |

无需其他系统级依赖。

---

## 🚀 快速开始

```bash
# 1. 安装依赖
npm install
# 国内网络推荐使用镜像加速
# npm install --registry=https://registry.npmmirror.com

# 2. 启动开发服务器（HMR；/dev-api 反代到 http://127.0.0.1:18080）
npm run dev
# → http://localhost:9528

# 3. 生产构建（产物输出到 ./dist）
npm run build:prod

# 4. 测试环境构建
npm run build:stage
```

> 💡 开发态下所有请求都带 `/dev-api` 前缀，并通过 `vue.config.js` 中的代理转发到后端；
> 生产构建不带前缀，由后端直接接管 `/api/...`。

---

## 🗂️ 目录结构

```
web/
├── public/                  # 原样拷贝的静态资源
├── src/
│   ├── api/                 # 按后端域拆分的 API 模块（user.js, device.js, …）
│   ├── assets/              # 图片、字体
│   ├── components/          # 通用组件
│   ├── directive/           # 自定义 Vue 指令
│   ├── icons/               # SVG 图标（symbol 方式）
│   ├── layout/              # 顶栏、侧栏、面包屑、标签栏…
│   ├── router/              # 路由 + 权限守卫
│   ├── store/               # vuex 模块
│   ├── styles/              # 全局 SCSS
│   ├── utils/               # request.js (axios 封装)、auth、formatters
│   ├── views/               # 页面级组件
│   ├── App.vue              # 根组件
│   ├── main.js              # 启动入口
│   ├── permission.js        # 路由守卫
│   └── settings.js          # 应用标题、Logo、固定顶栏等配置
├── tests/                   # Jest 单元测试
├── mock/                    # 可选的 mock 服务
├── .env.development         # ENV=development, VUE_APP_BASE_API=/dev-api
├── .env.production          # ENV=production,  VUE_APP_BASE_API=
├── vue.config.js            # devServer 代理、别名、webpack 调优
├── babel.config.js
├── postcss.config.js
├── jest.config.js
└── package.json
```

---

## 🔌 与后端的对接

| 维度 | 开发态 | 生产态 |
|------|--------|--------|
| API 前缀 | `/dev-api`（反代到 `http://127.0.0.1:18080`） | `""`（由后端直接服务 `/api/...`） |
| 配置源 | `web/.env.development` 中 `VUE_APP_BASE_API` | `web/.env.production` 中 `VUE_APP_BASE_API` |
| 反代规则 | `web/vue.config.js` → `devServer.proxy` | 不涉及 |
| 静态托管 | 后端 `static_dir = web/dist` | 后端 `static_dir = web/dist` |

**让 dev server 指向远端后端** —— 修改 `vue.config.js`：

```js
devServer: {
  proxy: {
    '/dev-api':  { target: 'http://your-backend:18080', changeOrigin: true, pathRewrite: { '^/dev-api': '/' } },
    '/static/snap': { target: 'http://your-backend:18080', changeOrigin: true },
  }
}
```

**鉴权** — `src/utils/request.js` 会自动从 cookie / 本地存储中读取 `access-token`
并附加到每个请求；返回体为 `{ code, msg, data }`，`code !== 0` 走统一错误提示。

---

## 📦 构建与发布

```bash
# 标准生产构建 → web/dist
npm run build:prod

# 测试环境构建（使用 .env.staging）
npm run build:stage

# 本地预览构建产物
npm run preview
npm run preview -- --report    # 同时输出打包体积分析

# 优化 SVG 图标
npm run svgo
```

`build:prod` 完成后**无需手动拷贝**，把后端 `static_dir` 指向 `web/dist` 即可。

---

## 🧹 代码规范

```bash
npm run lint            # 检查
npm run lint -- --fix   # 自动修复
```

- ESLint 配置 `plugin:vue/recommended`。
- 默认未集成 Prettier，可在编辑器中配置或自行加入 `eslint-config-prettier`。
- Vue 文件风格：2 空格缩进、单引号、无分号（参考 `eslintrc.js`）。

---

## 🧪 测试

```bash
npm run test:unit       # Jest + @vue/test-utils（先清缓存再跑）
npm run test:ci         # lint + 单元测试，CI 友好
```

测试文件放在 `tests/unit/**`，被 `jest.config.js` 自动发现。新增测试建议与源码路径保持一致。

---

## 🌍 国际化

原始模板以中文为主，本仓库继承了这一约定，**少量用户可见的英文文案散落在页面中**。
新增语言通常需要：

1. 抽离文案到 `src/lang/{en-US,zh-CN}.js`（建议引入 `vue-i18n`）。
2. 在 `src/main.js` 中注册并通过 vuex + `localStorage` 持久化选择。
3. 在 `src/components/LangSelect/` 添加语言切换入口。

---

## 🛠️ 常见问题

| 现象 | 可能原因 | 解决办法 |
|------|----------|----------|
| 开发态登录 404 | 后端未启动或端口不是 18080 | 仓库根目录 `cargo run` |
| `npm run dev` 打开空白页 | 9528 端口被占用 | `npm run dev -- --port 9529` |
| 生产构建无静态资源 | `web/dist` 为空 / `static_dir` 错配 | 重跑 `npm run build:prod`；后端配置 `static_dir = "web/dist"` |
| ESLint 保存报错 | 配置默认不开启保存时检查 | 手动 `npm run lint -- --fix` |
| 国内 `npm install` 慢 | 镜像源问题 | `npm install --registry=https://registry.npmmirror.com` |

---

## 🙏 致谢

本前端是基于
[vue-admin-template](https://github.com/PanJiaChen/vue-admin-template) 的深度定制版本，
原作者 **[PanJiaChen](https://github.com/PanJiaChen)**，MIT 协议。同源项目还包括：

- [vue-element-admin](https://github.com/PanJiaChen/vue-element-admin)
- [electron-vue-admin](https://github.com/PanJiaChen/electron-vue-admin)
- [vue-typescript-admin-template](https://github.com/Armour/vue-typescript-admin-template)

---

## 📜 许可证

MIT — 与上游 `vue-admin-template` 及 GBServer 项目保持一致。

---

<div align="center">

[← 返回 GBServer 根 README](../README.md) · [English →](./README.md)

</div>
