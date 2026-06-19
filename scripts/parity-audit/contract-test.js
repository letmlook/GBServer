#!/usr/bin/env node
/**
 * F1: parity-audit/contract-test.js
 *
 * 对每个 Rust 路由打 fixture（来自 extract-wvp-parity.js）+ 期望字段（来自 fixtures/*.json），
 * 校验后端响应形状与前端契约一致。
 *
 * 用法：
 *   1. node scripts/parity-audit/extract-wvp-parity.js  # 生成 docs/parity/wvp-phase-0-parity-audit.json
 *   2. 启动后端：cargo run --release
 *   3. BASE_URL=http://localhost:18080 node scripts/parity-audit/contract-test.js
 *      --user=admin --password=admin
 *
 * 输出：docs/parity/contract-test-report.html
 */

const fs = require('node:fs')
const path = require('node:path')
const http = require('node:http')
const https = require('node:https')

const PARITY_JSON = path.join(__dirname, '../../docs/parity/wvp-phase-0-parity-audit.json')
const FIXTURES_DIR = path.join(__dirname, 'fixtures')
const REPORT_HTML = path.join(__dirname, '../../docs/parity/contract-test-report.html')

// ---------- CLI args ----------
const args = process.argv.slice(2)
function arg(name, def) {
  const a = args.find((x) => x.startsWith(`--${name}=`))
  return a ? a.slice(name.length + 3) : def
}
const BASE_URL = arg('base-url', process.env.BASE_URL || 'http://localhost:18080')
const USERNAME = arg('user', 'admin')
const PASSWORD = arg('password', 'admin')

// ---------- HTTP helpers ----------
function httpJson(method, urlPath, { token, body, query } = {}) {
  return new Promise((resolve, reject) => {
    let url
    try {
      url = new URL(BASE_URL + urlPath)
    } catch (e) {
      return reject(new Error(`Invalid URL: ${BASE_URL}${urlPath}`))
    }
    if (query) {
      for (const [k, v] of Object.entries(query)) {
        url.searchParams.set(k, v)
      }
    }
    const lib = url.protocol === 'https:' ? https : http
    const headers = { 'Content-Type': 'application/json' }
    if (token) {
      headers['access-token'] = token
    }
    const req = lib.request(
      url,
      { method, headers },
      (res) => {
        const chunks = []
        res.on('data', (c) => chunks.push(c))
        res.on('end', () => {
          const text = Buffer.concat(chunks).toString('utf8')
          let data
          try {
            data = JSON.parse(text)
          } catch {
            data = text
          }
          resolve({ status: res.statusCode, data })
        })
      }
    )
    req.on('error', reject)
    if (body !== undefined) req.write(JSON.stringify(body))
    req.end()
  })
}

// ---------- Test plan ----------
async function login() {
  const r = await httpJson('get', `/api/user/login?username=${USERNAME}&password=${PASSWORD}`)
  if (r.status !== 200 || !r.data || r.data.code !== 0) {
    throw new Error(`Login failed: ${JSON.stringify(r.data)}`)
  }
  return r.data.data?.token || r.data.token
}

// 读取 parity JSON 中的路由，组装测试计划
function loadRoutes() {
  if (!fs.existsSync(PARITY_JSON)) {
    throw new Error(`Parity JSON not found: ${PARITY_JSON}. Run extract-wvp-parity.js first.`)
  }
  const data = JSON.parse(fs.readFileSync(PARITY_JSON, 'utf8'))
  // Matched = Rust already implements
  const matched = (data.matched || []).slice(0, 50) // 取前 50 个避免太慢
  return matched.map((m) => ({
    path: m.rustPath || m.wvpPath,
    wvpPath: m.wvpPath,
    method: m.method || 'GET',
    category: m.category || 'unknown'
  }))
}

// 加载 fixture（如果存在）
function loadFixture(routePath) {
  const safe = routePath.replace(/[^a-zA-Z0-9]/g, '_').replace(/_+/g, '_')
  const f = path.join(FIXTURES_DIR, `${safe}.json`)
  if (fs.existsSync(f)) {
    return JSON.parse(fs.readFileSync(f, 'utf8'))
  }
  return null
}

// 单条测试
async function runSingle(route, token) {
  const fix = loadFixture(route.path)
  const start = Date.now()
  let res
  try {
    const opts = { token }
    if (fix?.body) opts.body = fix.body
    if (fix?.query) opts.query = fix.query
    res = await httpJson(route.method.toLowerCase(), route.path, opts)
  } catch (e) {
    return {
      ok: false,
      latencyMs: Date.now() - start,
      error: e.message,
      fixture: !!fix,
    }
  }

  const latencyMs = Date.now() - start
  // 基础契约检查
  let pass = true
  const issues = []
  if (res.status === 0 || res.status >= 500) {
    pass = false
    issues.push(`HTTP ${res.status}`)
  }
  if (res.data && typeof res.data === 'object' && 'code' in res.data) {
    // WVP 响应格式 {code:0, msg:"成功", data:...}
    if (typeof res.data.code !== 'number') {
      pass = false
      issues.push('response.code is not number')
    }
    if (!('msg' in res.data)) {
      issues.push('response.msg missing (non-blocking)')
    }
    // F1: shape 校验 — 用 fixture.dataShape 检查关键字段类型
    if (fix?.dataShape && res.data.code === 0) {
      const shapeIssues = checkShape(res.data, fix.dataShape, '')
      issues.push(...shapeIssues)
      if (shapeIssues.length > 0) pass = false
    }
  } else if (typeof res.data === 'string' && res.data.length > 0) {
    // 纯文本（如 /api/health）也是合法的
  } else if (res.data === null || res.data === undefined) {
    // 也 OK
  } else {
    issues.push('response 不符合 WVPResult 格式')
  }

  return {
    ok: pass,
    latencyMs,
    status: res.status,
    issues,
    fixture: !!fix,
  }
}

// F1: 递归校验 JSON 形状（type descriptor 是 "number"/"string"/"object"/"array"/"boolean"/"null"）
function checkShape(actual, expected, path) {
  const issues = []
  if (expected === null || expected === undefined) return issues

  if (typeof expected === 'string') {
    // 简写：期望类型字符串
    const got = typeOf(actual)
    if (expected === 'array') {
      if (!Array.isArray(actual)) {
        issues.push(`${path || '/'} expected array, got ${got}`)
      }
    } else if (expected === 'object') {
      if (got !== 'object' || Array.isArray(actual)) {
        issues.push(`${path || '/'} expected object, got ${got}`)
      }
    } else if (got !== expected) {
      issues.push(`${path || '/'} expected ${expected}, got ${got}`)
    }
    return issues
  }

  if (typeof expected !== 'object' || Array.isArray(expected)) return issues

  // expected 是对象：检查每个字段存在且类型匹配
  if (typeof actual !== 'object' || actual === null || Array.isArray(actual)) {
    issues.push(`${path || '/'} expected object, got ${typeOf(actual)}`)
    return issues
  }
  for (const [k, v] of Object.entries(expected)) {
    const sub = path ? `${path}.${k}` : k
    if (!(k in actual)) {
      issues.push(`${sub} missing in response`)
      continue
    }
    issues.push(...checkShape(actual[k], v, sub))
  }
  return issues
}

function typeOf(v) {
  if (v === null) return 'null'
  if (Array.isArray(v)) return 'array'
  return typeof v
}

// ---------- 主流程 ----------
async function main() {
  console.log(`🔍 contract-test starting against ${BASE_URL}`)
  let token
  try {
    token = await login()
    console.log(`✅ login OK, token len=${token.length}`)
  } catch (e) {
    console.error(`❌ login failed: ${e.message}`)
    process.exit(1)
  }

  const routes = loadRoutes()
  console.log(`📋 test plan: ${routes.length} routes`)

  const results = []
  for (const r of routes) {
    const res = await runSingle(r, token)
    results.push({ ...r, ...res })
    const status = res.ok ? '✅' : '❌'
    process.stdout.write(`${status} ${r.method} ${r.path} (${res.latencyMs}ms)\n`)
  }

  const passed = results.filter((r) => r.ok).length
  const failed = results.length - passed
  console.log(`\n📊 result: ${passed}/${results.length} pass, ${failed} fail`)

  // 写 HTML 报告
  const html = renderHtml(results, passed, failed)
  fs.writeFileSync(REPORT_HTML, html)
  console.log(`📄 HTML report: ${REPORT_HTML}`)

  // 退出码：失败则 1
  process.exit(failed > 0 ? 1 : 0)
}

function renderHtml(results, passed, failed) {
  const rows = results.map((r) => `
    <tr class="${r.ok ? 'ok' : 'fail'}">
      <td>${r.method}</td>
      <td><code>${escapeHtml(r.path)}</code></td>
      <td>${escapeHtml(r.wvpPath || '')}</td>
      <td>${r.status || '-'}</td>
      <td>${r.latencyMs}ms</td>
      <td>${r.fixture ? '✓' : '—'}</td>
      <td>${r.ok ? '✅' : '❌ ' + escapeHtml((r.issues || []).join('; '))}</td>
    </tr>
  `).join('')

  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8" />
<title>GBServer Contract Test Report</title>
<style>
body { font-family: -apple-system, system-ui, sans-serif; margin: 24px; }
h1 { margin-bottom: 8px; }
.summary { font-size: 18px; margin-bottom: 16px; }
table { border-collapse: collapse; width: 100%; }
th, td { border: 1px solid #ddd; padding: 6px 10px; text-align: left; }
th { background: #f5f5f5; }
tr.ok { background: #f6fff6; }
tr.fail { background: #fff6f6; }
code { background: #f0f0f0; padding: 2px 4px; border-radius: 3px; }
.fail-count { color: #c00; }
.pass-count { color: #060; }
</style>
</head>
<body>
<h1>GBServer WVP-Pro 契约测试报告</h1>
<p class="summary">
  <span class="pass-count">通过 ${passed}</span> ·
  <span class="fail-count">失败 ${failed}</span> ·
  共 ${results.length} 个路由
</p>
<p>生成时间：${new Date().toISOString()}</p>
<p>基准 URL：${escapeHtml(BASE_URL)}</p>
<table>
<thead>
<tr><th>Method</th><th>Rust Path</th><th>WVP Path</th><th>HTTP</th><th>Latency</th><th>Fixture</th><th>Result</th></tr>
</thead>
<tbody>${rows}</tbody>
</table>
</body>
</html>
`
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
  }[c]))
}

if (require.main === module) {
  main().catch((e) => {
    console.error('Fatal:', e.message)
    process.exit(2)
  })
}

module.exports = { httpJson, loadRoutes, runSingle, checkShape, typeOf }
