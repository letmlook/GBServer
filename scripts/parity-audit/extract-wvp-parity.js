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

function findBalancedRange(text, startIndex, openChar, closeChar) {
  let depth = 0
  let quote = null
  let escaping = false

  for (let i = startIndex; i < text.length; i += 1) {
    const char = text[i]

    if (quote) {
      if (escaping) {
        escaping = false
      } else if (char === '\\') {
        escaping = true
      } else if (char === quote) {
        quote = null
      }
      continue
    }

    if (char === '"' || char === "'" || char === '`') {
      quote = char
      continue
    }

    if (char === openChar) {
      depth += 1
    } else if (char === closeChar) {
      depth -= 1
      if (depth === 0) {
        return { start: startIndex, end: i, body: text.slice(startIndex + 1, i) }
      }
    }
  }

  return null
}

function extractAnnotationArguments(annotationText) {
  const openIndex = annotationText.indexOf('(')
  if (openIndex === -1) return ''
  return findBalancedRange(annotationText, openIndex, '(', ')')?.body || ''
}

function extractNamedValueExpression(argsText) {
  const namedMatch = /\b(?:value|path)\s*=/.exec(argsText)
  if (!namedMatch) return ''

  let index = namedMatch.index + namedMatch[0].length
  while (/\s/.test(argsText[index] || '')) index += 1

  if (argsText[index] === '{') {
    return findBalancedRange(argsText, index, '{', '}')?.body || ''
  }

  if (argsText[index] === '"') {
    const stringMatch = argsText.slice(index).match(/^"((?:\\.|[^"])*)"/)
    return stringMatch ? `"${stringMatch[1]}"` : ''
  }

  return ''
}

function extractQuotedStrings(text) {
  return [...text.matchAll(/"((?:\\.|[^"])*)"/g)].map((match) => match[1])
}

function extractAnnotationValues(annotationText) {
  const argsText = extractAnnotationArguments(annotationText)
  if (!argsText) return ['']

  const trimmedArgs = argsText.trim()
  if (trimmedArgs.startsWith('{')) {
    const directArray = findBalancedRange(trimmedArgs, 0, '{', '}')
    if (directArray) return extractQuotedStrings(directArray.body)
  }

  const namedValue = extractNamedValueExpression(argsText)
  if (namedValue) return extractQuotedStrings(namedValue)

  return extractQuotedStrings(trimmedArgs)[0] ? [extractQuotedStrings(trimmedArgs)[0]] : ['']
}

function extractAnnotationValue(annotationText) {
  return extractAnnotationValues(annotationText)[0] || ''
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

  return annotationName === 'RequestMapping' ? ['ANY'] : ['GET']
}

function parseAnnotationAt(text, atIndex) {
  const nameMatch = text.slice(atIndex).match(/^@([A-Za-z_][A-Za-z0-9_]*)/)
  if (!nameMatch) return null

  let index = atIndex + nameMatch[0].length
  while (/\s/.test(text[index] || '')) index += 1

  if (text[index] === '(') {
    const args = findBalancedRange(text, index, '(', ')')
    if (!args) return null
    index = args.end + 1
  }

  return {
    name: nameMatch[1],
    text: text.slice(atIndex, index),
    end: index,
  }
}

function skipAnnotationBlocks(text, startIndex) {
  let index = startIndex

  while (index < text.length) {
    while (/\s/.test(text[index] || '')) index += 1
    if (text[index] !== '@') break

    const annotation = parseAnnotationAt(text, index)
    if (!annotation) break
    index = annotation.end
  }

  return index
}

function extractJavaControllerRoutesFromSource(source, sourcePath = '') {
  const clean = stripComments(source)
  const classMappingMatch = clean.match(/@(RequestMapping)\s*(\([^)]*\))?[\s\S]{0,500}?\bclass\s+\w+/)
  const classPrefixes = classMappingMatch ? extractAnnotationValues(classMappingMatch[0]) : ['']
  const routes = []
  const routeAnnotationPattern = /@(GetMapping|PostMapping|DeleteMapping|PutMapping|PatchMapping|RequestMapping)\b/g
  const methodDeclarationPattern = /^(?:public|private|protected)(?!\s+class)\s+[\w<>\[\].?,\s]+\s+\w+\s*\(/
  let match

  while ((match = routeAnnotationPattern.exec(clean)) !== null) {
    const annotation = parseAnnotationAt(clean, match.index)
    if (!annotation) continue

    routeAnnotationPattern.lastIndex = annotation.end
    const declarationIndex = skipAnnotationBlocks(clean, annotation.end)
    if (!methodDeclarationPattern.test(clean.slice(declarationIndex))) continue

    const annotationName = annotation.name
    const annotationText = annotation.text
    const methodPaths = extractAnnotationValues(annotationText)
    const methods = extractRequestMethods(annotationName, annotationText)
    for (const classPrefix of classPrefixes) {
      for (const methodPath of methodPaths) {
        for (const method of methods) {
          routes.push({
            method,
            path: joinRouteParts(classPrefix, methodPath),
            source: sourcePath,
            kind: 'java-controller',
          })
        }
      }
    }
  }
  return routes.sort((a, b) => (
    a.path.localeCompare(b.path)
    || a.method.localeCompare(b.method)
    || a.source.localeCompare(b.source)
    || a.kind.localeCompare(b.kind)
  ))
}

function extractRustRouterRoutesFromSource(source, sourcePath = '') {
  const clean = stripComments(source)
  const routes = []
  const routePattern = /\.route\(\s*"([^"]+)"\s*,\s*([\s\S]*?)\)\s*(?=\.route|\.merge|\.fallback|\.layer|\.with_state|;|$)/g
  let match
  while ((match = routePattern.exec(clean)) !== null) {
    const routePath = normalizeRoutePath(match[1])
    const handlerExpression = match[2]
    const methods = new Set()
    const methodPattern = /\b(get|post|delete|put|patch)\s*\(/g
    let methodMatch
    while ((methodMatch = methodPattern.exec(handlerExpression)) !== null) {
      methods.add(methodMatch[1].toUpperCase())
    }
    for (const method of methods) {
      routes.push({
        method,
        path: routePath,
        source: sourcePath,
        kind: 'rust-router',
      })
    }
  }
  return routes.sort((a, b) => `${a.method} ${a.path}`.localeCompare(`${b.method} ${b.path}`))
}

function normalizeFrontendUrlExpression(expression) {
  const parts = String(expression)
    .split('+')
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => {
      const stringMatch = part.match(/^'([^']*)'$|^"([^"]*)"$|^`([^`]*)`$/)
      if (stringMatch) return stringMatch[1] || stringMatch[2] || stringMatch[3]
      return '{dynamic}'
    })

  if (parts.length === 0) return '/{dynamic}'

  const collapsed = parts
    .join('')
    .replace(/\$\{[^}]+\}/g, '{dynamic}')
    .replace(/\/+/g, '/')

  return normalizeRoutePath(collapsed)
}

function readObjectPropertyExpression(objectBody, propertyName) {
  const propertyPattern = new RegExp(`\\b${propertyName}\\s*:`)
  const propertyMatch = propertyPattern.exec(objectBody)
  if (!propertyMatch) return ''

  let index = propertyMatch.index + propertyMatch[0].length
  while (/\s/.test(objectBody[index] || '')) index += 1

  const start = index
  const stack = []
  let quote = null
  let escaping = false

  for (; index < objectBody.length; index += 1) {
    const char = objectBody[index]

    if (quote) {
      if (escaping) {
        escaping = false
      } else if (char === '\\') {
        escaping = true
      } else if (char === quote) {
        quote = null
      }
      continue
    }

    if (char === '"' || char === "'" || char === '`') {
      quote = char
      continue
    }

    if (char === '(' || char === '[' || char === '{') {
      stack.push(char)
      continue
    }

    if (char === ')' || char === ']' || char === '}') {
      if (stack.length === 0) break
      stack.pop()
      continue
    }

    if (stack.length === 0 && (char === ',' || char === '\n')) break
  }

  return objectBody.slice(start, index).trim()
}

function extractFrontendApiCallsFromSource(source, sourcePath = '') {
  const clean = stripComments(source)
  const calls = []
  const requestPattern = /request\s*\(\s*\{/g
  let requestMatch
  while ((requestMatch = requestPattern.exec(clean)) !== null) {
    const openBraceIndex = clean.indexOf('{', requestMatch.index)
    const objectRange = findBalancedRange(clean, openBraceIndex, '{', '}')
    if (!objectRange) continue

    requestPattern.lastIndex = objectRange.end
    const urlExpression = readObjectPropertyExpression(objectRange.body, 'url')
    if (!urlExpression) continue

    const methodMatch = objectRange.body.match(/\bmethod\s*:\s*['"]([A-Za-z]+)['"]/)
    const method = methodMatch ? methodMatch[1].toUpperCase() : 'GET'
    calls.push({
      method,
      path: normalizeFrontendUrlExpression(urlExpression),
      source: sourcePath,
      kind: 'frontend-api',
    })
  }
  return calls.sort((a, b) => `${a.path} ${a.method}`.localeCompare(`${b.path} ${b.method}`))
}

function findEnclosingObjectRange(text, index) {
  const stack = []
  let quote = null
  let escaping = false

  for (let i = 0; i < index; i += 1) {
    const char = text[i]

    if (quote) {
      if (escaping) {
        escaping = false
      } else if (char === '\\') {
        escaping = true
      } else if (char === quote) {
        quote = null
      }
      continue
    }

    if (char === '"' || char === "'" || char === '`') {
      quote = char
      continue
    }

    if (char === '{') {
      stack.push(i)
    } else if (char === '}') {
      stack.pop()
    }
  }

  for (let i = stack.length - 1; i >= 0; i -= 1) {
    const range = findBalancedRange(text, stack[i], '{', '}')
    if (range && range.end >= index) return range
  }

  return null
}

function extractRouteObjectStringProperty(objectBody, propertyName) {
  const expression = readObjectPropertyExpression(objectBody, propertyName)
  const stringMatch = expression.match(/^['"]([^'"]+)['"]$/)
  return stringMatch ? stringMatch[1] : ''
}

function extractVueRouterPagesFromSource(source, sourcePath = '') {
  const clean = stripComments(source)
  const pages = []
  const seen = new Set()
  const componentPattern = /component\s*:\s*\(\)\s*=>\s*import\(\s*['"]([^'"]+)['"]\s*\)/g
  let match
  while ((match = componentPattern.exec(clean)) !== null) {
    const routeObject = findEnclosingObjectRange(clean, match.index)
    if (!routeObject) continue

    const routePath = extractRouteObjectStringProperty(routeObject.body, 'path')
    const routeName = extractRouteObjectStringProperty(routeObject.body, 'name')
    if (!routePath || !routeName) continue

    const page = {
      path: normalizeRoutePath(routePath),
      name: routeName,
      component: match[1],
      source: sourcePath,
      kind: 'frontend-page',
    }
    const key = `${page.path}\0${page.name}\0${page.component}`
    if (seen.has(key)) continue
    seen.add(key)
    pages.push(page)
  }
  return pages.sort((a, b) => `${a.path} ${a.name}`.localeCompare(`${b.path} ${b.name}`))
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
  extractAnnotationValues,
  extractRequestMethods,
  extractJavaControllerRoutesFromSource,
  extractRustRouterRoutesFromSource,
  normalizeFrontendUrlExpression,
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
