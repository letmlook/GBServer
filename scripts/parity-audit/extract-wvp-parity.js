#!/usr/bin/env node

const fs = require('node:fs')
const path = require('node:path')

function normalizeRoutePath(routePath) {
  if (!routePath) return '/'
  let normalized = String(routePath).trim()
  if (!normalized.startsWith('/')) normalized = `/${normalized}`
  normalized = normalized.replace(/\/+/g, '/')
  normalized = normalized.replace(/\/$/, '') || '/'
  normalized = normalized.replace(/:([A-Za-z_][A-Za-z0-9_]*)/g, '{$1}')
  return normalized
}

function extractJavaControllerRoutesFromSource() {
  return []
}

function extractRustRouterRoutesFromSource() {
  return []
}

function extractFrontendApiCallsFromSource() {
  return []
}

function extractVueRouterPagesFromSource() {
  return []
}

function compareRouteSets(referenceRoutes, targetRoutes) {
  return {
    aligned: [],
    missing: referenceRoutes.slice(),
    extra: targetRoutes.slice(),
    methodMismatch: [],
  }
}

function buildMarkdownReport(audit) {
  return `# WVP-Pro Phase 0 Parity Audit\n\nBaseline: ${audit.baseline?.commit || 'unknown'}\n`
}

function walkFiles(rootDir, predicate) {
  if (!fs.existsSync(rootDir)) return []
  const results = []
  const stack = [rootDir]
  while (stack.length > 0) {
    const current = stack.pop()
    const stat = fs.statSync(current)
    if (stat.isDirectory()) {
      for (const entry of fs.readdirSync(current)) {
        if (entry === 'node_modules' || entry === 'target' || entry === 'dist' || entry === '.git') continue
        stack.push(path.join(current, entry))
      }
    } else if (predicate(current)) {
      results.push(current)
    }
  }
  return results.sort()
}

function main(argv) {
  const args = parseArgs(argv)
  const audit = {
    baseline: {
      upstream: args.upstream,
      local: args.local,
      commit: args.commit || 'unknown',
    },
    generatedAt: new Date().toISOString(),
    javaRoutes: [],
    rustRoutes: [],
    upstreamFrontendApi: [],
    localFrontendApi: [],
    upstreamPages: [],
    localPages: [],
    comparisons: {},
  }
  const markdown = buildMarkdownReport(audit)
  if (args.outDir) {
    fs.mkdirSync(args.outDir, { recursive: true })
    fs.writeFileSync(path.join(args.outDir, 'wvp-phase-0-parity-audit.json'), `${JSON.stringify(audit, null, 2)}\n`)
    fs.writeFileSync(path.join(args.outDir, 'wvp-phase-0-parity-audit.md'), markdown)
  } else {
    process.stdout.write(markdown)
  }
}

function parseArgs(argv) {
  const args = {
    upstream: '/tmp/wvp-GB28181-pro',
    local: process.cwd(),
    outDir: 'docs/parity',
    commit: '',
  }
  for (let i = 0; i < argv.length; i += 1) {
    const key = argv[i]
    const value = argv[i + 1]
    if (key === '--upstream') {
      args.upstream = value
      i += 1
    } else if (key === '--local') {
      args.local = value
      i += 1
    } else if (key === '--out-dir') {
      args.outDir = value
      i += 1
    } else if (key === '--commit') {
      args.commit = value
      i += 1
    }
  }
  return args
}

module.exports = {
  normalizeRoutePath,
  extractJavaControllerRoutesFromSource,
  extractRustRouterRoutesFromSource,
  extractFrontendApiCallsFromSource,
  extractVueRouterPagesFromSource,
  compareRouteSets,
  buildMarkdownReport,
  walkFiles,
  parseArgs,
}

if (require.main === module) {
  main(process.argv.slice(2))
}
