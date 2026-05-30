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

function stripComments(source) {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/(^|[^:])\/\/.*$/gm, '$1')
}

function joinRouteParts(prefix, suffix) {
  const left = normalizeRoutePath(prefix || '/')
  const right = normalizeRoutePath(suffix || '/')
  if (left === '/') return right
  if (right === '/') return left
  return normalizeRoutePath(`${left}/${right.replace(/^\//, '')}`)
}

function extractAnnotationValue(annotationText) {
  const directString = annotationText.match(/\(\s*"([^"]*)"\s*\)/)
  if (directString) return directString[1]

  const valueString = annotationText.match(/\b(?:value|path)\s*=\s*"([^"]*)"/)
  if (valueString) return valueString[1]

  return ''
}

function extractRequestMethods(annotationName, annotationText) {
  const fixed = {
    GetMapping: ['GET'],
    PostMapping: ['POST'],
    DeleteMapping: ['DELETE'],
    PutMapping: ['PUT'],
    PatchMapping: ['PATCH'],
  }
  if (fixed[annotationName]) return fixed[annotationName]

  const methodBlock = annotationText.match(/method\s*=\s*\{([^}]+)\}/)
  if (methodBlock) {
    return methodBlock[1]
      .split(',')
      .map((part) => part.trim().replace(/^RequestMethod\./, ''))
      .filter(Boolean)
  }

  const singleMethod = annotationText.match(/method\s*=\s*RequestMethod\.([A-Z]+)/)
  if (singleMethod) return [singleMethod[1]]

  return ['GET']
}

function extractJavaControllerRoutesFromSource(source, sourcePath = '') {
  const clean = stripComments(source)
  const classMappingMatch = clean.match(/@(RequestMapping)\s*(\([^)]*\))?[\s\S]{0,500}?\bclass\s+\w+/)
  const classPrefix = classMappingMatch ? extractAnnotationValue(classMappingMatch[0]) : ''
  const routes = []
  const routeAnnotationPattern = /@(GetMapping|PostMapping|DeleteMapping|PutMapping|PatchMapping|RequestMapping)\s*(\([^)]*\))?\s*(?:\r?\n\s*)*(?:public|private|protected|@Operation|@Parameter|@ApiOperation)/g
  let match
  while ((match = routeAnnotationPattern.exec(clean)) !== null) {
    if (/^\s+class\b/.test(clean.slice(routeAnnotationPattern.lastIndex, routeAnnotationPattern.lastIndex + 50))) continue
    const annotationName = match[1]
    const annotationText = match[0]
    const methodPath = extractAnnotationValue(annotationText)
    const methods = extractRequestMethods(annotationName, annotationText)
    for (const method of methods) {
      routes.push({
        method,
        path: joinRouteParts(classPrefix, methodPath),
        source: sourcePath,
        kind: 'java-controller',
      })
    }
  }
  return routes.sort((a, b) => (
    a.path.localeCompare(b.path)
    || a.method.localeCompare(b.method)
    || a.source.localeCompare(b.source)
    || a.kind.localeCompare(b.kind)
  ))
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
  stripComments,
  joinRouteParts,
  extractAnnotationValue,
  extractRequestMethods,
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
