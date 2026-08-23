# GBServer Web · Frontend

The web management console for **[GBServer](../README.md)** — a Rust-based GB/T 28181 video platform.

It is a **Vue 2 + Element UI** single-page application that talks to the GBServer HTTP API
(`/api/*`) and provides day-to-day operations for devices, channels, streams, recording plans,
cascade platforms, JT1078 vehicles, and user/system administration.

The frontend pages mirror the conventional GB28181 admin console: any feature exposed by the
backend HTTP API is reachable from the UI without further glue code.

> 🇨🇳 [中文说明](./README-zh.md)

---

## 📑 Contents

- [Overview](#-overview)
- [Tech Stack](#-tech-stack)
- [Prerequisites](#-prerequisites)
- [Quick Start](#-quick-start)
- [Project Layout](#-project-layout)
- [Integration with Backend](#-integration-with-backend)
- [Build & Release](#-build--release)
- [Code Style & Quality](#-code-style--quality)
- [Testing](#-testing)
- [Internationalization](#-internationalization)
- [Troubleshooting](#-troubleshooting)
- [Acknowledgements](#-acknowledgements)
- [License](#-license)

---

## 🔭 Overview

- **Purpose** — UI for GBServer. Implements the conventional GB28181 admin screens and call sites,
  so any feature available in the backend HTTP API is reachable here.
- **Mode** — SPA; backend serves the built static bundle from `web/dist` when `static_dir` is set.
- **Auth** — JWT in `access-token` header; admin login, role-based sidebar, dynamic routes.
- **Visualization** — ECharts / v-charts, OpenLayers (maps), log viewer, tree components.

---

## 🛠️ Tech Stack

| Layer | Library | Version |
|-------|---------|---------|
| Framework | Vue | 2.6.10 |
| UI Kit | Element UI | 2.15.x |
| Router | vue-router | 3.0.6 |
| State | vuex | 3.1.0 |
| HTTP | axios | ^0.24.0 |
| Charts | ECharts / v-charts | 4.9 / 1.19 |
| Maps | OpenLayers | ^10.6.1 |
| Trees | vue-ztree-2.0 / @wchbrad/vue-easy-tree | — |
| Date | dayjs / moment | 1.11 / 2.29 |
| Build | @vue/cli-service | 4.4.4 |
| Test | Jest + @vue/test-utils | 23 / 1.0-beta |
| Lint | ESLint + eslint-plugin-vue | 6.7 / 6.2 |

> Engines: Node `>=8.9`, npm `>=3.0.0`. Recommended: Node 16/18 LTS.

---

## ✅ Prerequisites

| Tool | Why | Install |
|------|-----|---------|
| **Node.js** ≥ 14 (16/18 LTS recommended) | Build & dev server | <https://nodejs.org/> |
| **npm** ≥ 6 (bundled with Node) | Dependency management | — |
| **GBServer backend** running on `:18080` | Data source for dev mode | see [../README.md §Quick Start](../README.md#-quick-start) |

No additional system libraries are required.

---

## 🚀 Quick Start

```bash
# 1. Install dependencies
npm install
# (in China, use a mirror to speed up)
# npm install --registry=https://registry.npmmirror.com

# 2. Start the dev server (with HMR, proxies /dev-api to http://127.0.0.1:18080)
npm run dev
# → http://localhost:9528

# 3. Build for production (output: ./dist)
npm run build:prod

# 4. Build for staging
npm run build:stage
```

> 💡 During dev, requests from the SPA are prefixed with `/dev-api` and proxied to the backend.
> In production builds, the prefix is empty so the bundle can be served by the backend itself.

---

## 🗂️ Project Layout

```
web/
├── public/                  # Static assets copied as-is
├── src/
│   ├── api/                 # One file per backend domain (user.js, device.js, …)
│   ├── assets/              # Images, fonts
│   ├── components/          # Reusable components
│   ├── directive/           # Custom Vue directives
│   ├── icons/               # SVG icon sprite
│   ├── layout/              # Header, Sidebar, Breadcrumb, TagsView, …
│   ├── router/              # vue-router definitions + permission guards
│   ├── store/               # vuex modules
│   ├── styles/              # Global SCSS
│   ├── utils/               # request.js (axios wrapper), auth, formatters
│   ├── views/               # Page-level components
│   ├── App.vue              # Root
│   ├── main.js              # Bootstrap
│   ├── permission.js        # Route guard
│   └── settings.js          # App title, logo, fixed-header flags
├── tests/                   # Jest unit tests
├── mock/                    # Optional mock server
├── .env.development         # ENV=development, VUE_APP_BASE_API=/dev-api
├── .env.production          # ENV=production,  VUE_APP_BASE_API=
├── vue.config.js            # devServer proxy, alias, webpack tweaks
├── babel.config.js
├── postcss.config.js
├── jest.config.js
└── package.json
```

---

## 🔌 Integration with Backend

| Aspect | Dev | Prod |
|--------|-----|------|
| Base API prefix | `/dev-api` (proxied to `http://127.0.0.1:18080`) | `""` (served by backend on `/api/...`) |
| Source of truth | `web/.env.development` → `VUE_APP_BASE_API` | `web/.env.production` → `VUE_APP_BASE_API` |
| Proxy config | `web/vue.config.js` → `devServer.proxy` | n/a |
| Static hosting | backend `static_dir` = `web/dist` | backend `static_dir` = `web/dist` |

**Pointing dev server at a remote backend** — edit `vue.config.js`:

```js
devServer: {
  proxy: {
    '/dev-api': { target: 'http://your-backend:18080', changeOrigin: true, pathRewrite: { '^/dev-api': '/' } },
    '/static/snap': { target: 'http://your-backend:18080', changeOrigin: true },
  }
}
```

**Auth** — `src/utils/request.js` attaches `access-token` from cookie/local storage on every
request and unwraps `{ code, msg, data }` responses. Non-zero `code` triggers a unified error toast.

---

## 📦 Build & Release

```bash
# Standard production build → web/dist
npm run build:prod

# Staging build (uses .env.staging)
npm run build:stage

# Preview the built bundle locally
npm run preview
npm run preview -- --report    # also produces bundle analysis

# Optimize SVG sprites
npm run svgo
```

After `build:prod`, copy nothing manually — just point GBServer's `static_dir` to `web/dist`
and restart the backend.

---

## 🧹 Code Style & Quality

```bash
npm run lint            # check
npm run lint -- --fix   # auto-fix
```

- ESLint with `plugin:vue/recommended`.
- Prettier is **not** included by default; integrate via your editor or add `eslint-config-prettier`.
- Vue files: 2-space indent, single quotes, no semicolons (per `eslintrc.js`).

---

## 🧪 Testing

```bash
npm run test:unit       # Jest with @vue/test-utils, clears cache first
npm run test:ci         # lint + unit tests, suitable for CI
```

Test files live in `tests/unit/**` and are picked up by `jest.config.js`. Add new specs next
to the code they cover or in `tests/unit/`, mirroring the source path.

---

## 🌍 Internationalization

The original vue-admin-template is Chinese-first; this fork keeps the same primary locale
with English text in a few user-facing labels. Adding a new language typically means:

1. Extract string literals into `src/lang/{en-US,zh-CN}.js` (vue-i18n recommended).
2. Register in `src/main.js` and persist choice in `vuex` + `localStorage`.
3. Add a language switcher under `src/components/LangSelect/`.

---

## 🛠️ Troubleshooting

| Symptom | Likely Cause | Fix |
|---------|--------------|-----|
| Login fails in dev with 404 on `/dev-api/...` | Backend not started on `:18080` | `cargo run` from repo root |
| `npm run dev` opens a blank page | Port 9528 occupied | `npm run dev -- --port 9529` |
| Production bundle not served | `web/dist` empty or wrong `static_dir` | Run `npm run build:prod`; set `static_dir = "web/dist"` |
| ESLint errors on save | Disabled by config to avoid noise | Run `npm run lint -- --fix` |
| China network slow install | `registry.npmjs.org` reachable | `npm install --registry=https://registry.npmmirror.com` |

---

## 🙏 Acknowledgements

This frontend is a heavily customized fork of
[vue-admin-template](https://github.com/PanJiaChen/vue-admin-template) by
**[PanJiaChen](https://github.com/PanJiaChen)** (MIT). All upstream credits apply.

Related projects in the same family:

- [vue-element-admin](https://github.com/PanJiaChen/vue-element-admin)
- [electron-vue-admin](https://github.com/PanJiaChen/electron-vue-admin)
- [vue-typescript-admin-template](https://github.com/Armour/vue-typescript-admin-template)

---

## 📜 License

MIT — same as the upstream `vue-admin-template` and the GBServer project.

---

<div align="center">

[← Back to GBServer root](../README.md) · [中文文档 →](./README-zh.md)

</div>
