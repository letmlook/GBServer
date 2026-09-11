# GBServer E2E (Playwright)

Headless Chromium-driven smoke tests for the GBServer admin SPA.

## Prerequisites (one-time)

The backend, frontend, and dependency services must be up:

```bash
# 1. Postgres + Redis + ZLMediaKit
docker compose up -d postgres redis zlm

# 2. Rust backend on :18080
cd /home/letmlook/GBServer
nohup ./target/release/gbserver > e2e/artifacts/backend.log 2>&1 &
echo $! > e2e/artifacts/backend.pid

# 3. Vue dev server on :9528
cd web
nohup npm run dev > /home/letmlook/GBServer/e2e/artifacts/frontend.log 2>&1 &
echo $! > /home/letmlook/GBServer/e2e/artifacts/frontend.pid
```

Wait for both ports to answer 200:

```bash
curl http://127.0.0.1:18080/api/health   # backend
curl -I http://127.0.0.1:9528/            # frontend
```

## Install (one-time)

```bash
cd e2e
npm install
npx playwright install chromium
```

## Run

```bash
npx playwright test                       # headless
npx playwright test --headed              # requires a graphical display
npx playwright show-report                # open the HTML report
```

Screenshots per page land in `e2e/artifacts/<name>.png`.
鉴权状态由 `global-setup.ts` 写入 `e2e/artifacts/.auth.json`（登录一次，
所有用例复用），与用例的执行顺序无关。

## 两条容易踩的坑（2026-09-12 修复）

1. **路由是 hash 模式**（`web/src/router/index.ts` 用 `createWebHashHistory`）。
   访问某个页面必须写 `/#/live`；写成 `/live` 时 hash 为空、vue-router 会解析成
   `/` 并重定向到 `#/dashboard`，于是"页面渲染"用例会在**控制台页**上通过。
   `smoke.spec.ts` 因此额外断言 `page.url()` 的 hash 确实切到了目标路由 ——
   没有这条断言时，17 个"page renders"用例会全部在渲染 dashboard 的情况下通过。
2. **鉴权状态必须先于任何用例存在**。此前 `artifacts/.auth.json` 是
   `smoke.spec.ts` 第 3 个用例的副作用，而 Playwright 按文件名字母序执行
   （`live.spec.ts` 在前），干净检出上 live 的 5 个用例必然因
   `ENOENT: artifacts/.auth.json` 失败。现已提升为 `globalSetup`。

## 前置服务

* 后端 :18080（`GBSERVER__DATABASE__URL` 可指向临时 SQLite）
* 前端 dev :9528（`/dev-api` 代理到 :18080；见 `web/vite.config.ts`）
* `npx playwright install chromium`

PTZ 用例需要数据库里至少有一个通道：可先跑
`mock/tools/sip-device/sip_device_mock.py` 注册设备并同步目录，或直接往
`gb_device_channel` 插一行；没有任何通道时该用例按条件跳过。

## Teardown

```bash
kill "$(cat e2e/artifacts/backend.pid)" 2>/dev/null
kill "$(cat e2e/artifacts/frontend.pid)" 2>/dev/null
docker compose down
```
