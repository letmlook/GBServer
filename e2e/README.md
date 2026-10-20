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

### 媒体节点的 IP 是**按机器写死**在 `config/application.toml` 里的

`[[zlm.servers]]` 的 `ip` 与 `rtc_extern_ip` 默认填的是某台开发机的 LAN IP。
换一台机器（或该机 IP 变了）就必须覆盖，否则：

* 媒体节点连不上 → 点播/拉流用例全挂，后端日志里是 ZLM 节点 Unknown；
* `rtc.externIP` 指向一个不可达地址 → 浏览器拿到的 ICE 候选连不上，
  `iceConnectionState` 永远停在 `checking`（**表现和 WebRTC 实现坏了很像**，
  但其实是地址问题）。

用这两个**标量**环境变量覆盖（不要用 `GBSERVER__ZLM__SERVERS__0__*` ——
那会让配置库把节点数组解析成 map，启动时报 `invalid type: map`）：

```bash
export GBSERVER_ZLM_PRIMARY_IP=127.0.0.1          # 后端 → ZLM HTTP
export GBSERVER_ZLM_PRIMARY_RTC_EXTERN_IP=<本机LAN IP>  # 浏览器 → ZLM RTC
```

浏览器要能连上 ZLM 的 RTC 端口，ZLM 容器需发布 UDP 8000（mac 叠加文件已配）。

## Run

```bash
npx playwright test                       # headless
npx playwright test --headed              # requires a graphical display
npx playwright show-report                # open the HTML report
npm run test:health                        # 依赖进程健康检查单测（Node 22）
```

后端直接托管 `web/dist` 时无需 Vite。验收用例可将业务 API 前缀显式设为
`/api`（默认 `/dev-api`，用于 Vite 代理）；必须有真实 ZLM 与持续在线的设备
或推 RTP 的 SIP Mock 才能运行媒体与录像用例，WebRTC 用例还需显式启用：

```bash
E2E_BASE_URL=http://127.0.0.1:18080 E2E_API_PREFIX=/api \
  npx playwright test tests/playUrls.spec.ts tests/cloudRecord.spec.ts
E2E_BASE_URL=http://127.0.0.1:18080 E2E_API_PREFIX=/api E2E_REQUIRE_WEBRTC=1 \
  npx playwright test tests/webrtcMedia.spec.ts
```

SIP Mock 若关闭自动心跳，后端可能在两个用例之间将其判为离线；连续运行时保留
默认心跳或设置 `--auto-keepalive 20`。

`tests/cloudRecord.spec.ts` 会创建一个短时录像计划，录制后删除该计划，并且只接受
本次新增、属于所选通道、文件大小大于零的 MP4 数据库记录。它会用这条记录验证页面、
播放/下载和删除。没有在线通道时才跳过；创建计划后未产出文件会失败。运行该用例
需要能收流的在线设备或 SIP Mock，以及可用的 ZLM。历史录像不能让用例通过。
运行前与录制等待期间会检查后端 HTTP health、ZLM TCP 端口；如果本轮由脚本
启动了 SIP Mock，可提供它的 PID 文件来检测中途退出。检查仅观察进程，不终止它们。

```bash
E2E_BACKEND_PID_FILE=artifacts/backend.pid \
E2E_BACKEND_LOG_FILE=artifacts/backend.log \
E2E_FRONTEND_PID_FILE=artifacts/frontend.pid \
E2E_FRONTEND_LOG_FILE=artifacts/frontend.log \
E2E_ZLM_BASE_URL=http://127.0.0.1:8080 \
E2E_SIP_MOCK_PID_FILE=artifacts/sip-mock.pid \
E2E_SIP_MOCK_LOG_FILE=artifacts/sip-mock.log \
E2E_SIP_MOCK_DEVICE_ID=34020000001320000001 \
npx playwright test tests/cloudRecord.spec.ts
```

PID 文件只填当前运行由脚本记录的 PID。远程后端可设置
`E2E_BACKEND_HEALTH_URL`；ZLM 不在本机时设置 `E2E_ZLM_BASE_URL`。
未提供 SIP Mock PID 文件表示用真机或未托管 Mock，本用例不会宣称已监测其进程。
`E2E_SIP_MOCK_DEVICE_ID` 仅在明确使用 Mock 时设置；用例会先调用设备目录同步，
因此临时空数据库不会因缺少通道而跳过。Mock 必须用设备国标 ID 作 Digest 用户名：

```bash
python3 mock/tools/sip-device/sip_device_mock.py \
  --server 127.0.0.1:5060 --username 34020000001320000001 \
  --send-rtp --auto-bye-secs 0
```

Mock 会优先选择本机 ffmpeg 的 `libopenh264`，没有时使用 `libx264`。需使用
真实 ZLM 才能验证 MP4 文件；`mock/tools/zlm/zlm_mock.py` 不会录制文件。
全量 Playwright 的报警页用例需要数据；Mock 可加 `--auto-alarm-secs 5`，否则这
两项会明确跳过。`E2E_REQUIRE_WEBRTC=1` 会让 WebRTC 用例实际协商并验证 ICE、
解码帧；未设置时该项按设计跳过。

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
