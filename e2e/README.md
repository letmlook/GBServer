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

## Teardown

```bash
kill "$(cat e2e/artifacts/backend.pid)" 2>/dev/null
kill "$(cat e2e/artifacts/frontend.pid)" 2>/dev/null
docker compose down
```
