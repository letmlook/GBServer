const assert = require('node:assert/strict')
const test = require('node:test')

const audit = require('../../scripts/parity-audit/extract-interface-coverage')

test('module exports parser and formatter functions', () => {
  assert.equal(typeof audit.normalizeRoutePath, 'function')
  assert.equal(typeof audit.extractJavaControllerRoutesFromSource, 'function')
  assert.equal(typeof audit.extractRustRouterRoutesFromSource, 'function')
  assert.equal(typeof audit.extractFrontendApiCallsFromSource, 'function')
  assert.equal(typeof audit.extractVueRouterPagesFromSource, 'function')
  assert.equal(typeof audit.compareRouteSets, 'function')
  assert.equal(typeof audit.buildMarkdownReport, 'function')
  assert.equal(typeof audit.walkFiles, 'function')
  assert.equal(typeof audit.parseArgs, 'function')
})

test('normalizeRoutePath converts Axum and Spring path params to a common shape', () => {
  assert.equal(audit.normalizeRoutePath('/api/play/start/:device_id/:channel_id'), '/api/play/start/{device_id}/{channel_id}')
  assert.equal(audit.normalizeRoutePath('/api/play/start/{deviceId}/{channelId}'), '/api/play/start/{deviceId}/{channelId}')
  assert.equal(audit.normalizeRoutePath('api/device/query/devices/{deviceId}/'), '/api/device/query/devices/{deviceId}')
  assert.equal(audit.normalizeRoutePath(''), '/')
})

test('extractJavaControllerRoutesFromSource reads class and method mappings', () => {
  const source = `
    package com.example;

    @RestController
    @RequestMapping("/api/play")
    public class PlayController {
      @GetMapping("/start/{deviceId}/{channelId}")
      public WVPResult start() { return null; }

      @PostMapping(value = "/stop/{deviceId}/{channelId}")
      public WVPResult stop() { return null; }

      @RequestMapping(value = "/ssrc", method = RequestMethod.GET)
      public WVPResult ssrc() { return null; }
    }
  `

  const routes = audit.extractJavaControllerRoutesFromSource(source, 'PlayController.java')

  assert.deepEqual(routes.map((route) => ({ method: route.method, path: route.path, source: route.source })), [
    { method: 'GET', path: '/api/play/ssrc', source: 'PlayController.java' },
    { method: 'GET', path: '/api/play/start/{deviceId}/{channelId}', source: 'PlayController.java' },
    { method: 'POST', path: '/api/play/stop/{deviceId}/{channelId}', source: 'PlayController.java' },
  ])
})

test('extractJavaControllerRoutesFromSource supports array mapping methods', () => {
  const source = `
    @RequestMapping(path = "/api/device/query")
    public class DeviceQuery {
      @RequestMapping(value = "/devices", method = {RequestMethod.GET, RequestMethod.POST})
      public WVPResult devices() { return null; }
    }
  `

  const routes = audit.extractJavaControllerRoutesFromSource(source, 'DeviceQuery.java')

  assert.deepEqual(routes.map((route) => `${route.method} ${route.path}`), [
    'GET /api/device/query/devices',
    'POST /api/device/query/devices',
  ])
})

test('extractJavaControllerRoutesFromSource emits one route per direct mapping path', () => {
  const source = `
    @RequestMapping("/api/device")
    public class DeviceController {
      @GetMapping({"/a", "/b"})
      public WVPResult aliases() { return null; }
    }
  `

  const routes = audit.extractJavaControllerRoutesFromSource(source, 'DeviceController.java')

  assert.deepEqual(routes.map((route) => `${route.method} ${route.path}`), [
    'GET /api/device/a',
    'GET /api/device/b',
  ])
})

test('extractJavaControllerRoutesFromSource emits one route per named RequestMapping path', () => {
  const source = `
    @RequestMapping(path = "/api/play")
    public class PlayController {
      @RequestMapping(value = {"/a", "/b"}, method = RequestMethod.GET)
      public WVPResult aliases() { return null; }
    }
  `

  const routes = audit.extractJavaControllerRoutesFromSource(source, 'PlayController.java')

  assert.deepEqual(routes.map((route) => `${route.method} ${route.path}`), [
    'GET /api/play/a',
    'GET /api/play/b',
  ])
})

test('extractJavaControllerRoutesFromSource emits one route per direct mapping path with path variables', () => {
  const source = `
    @RequestMapping("/api/device")
    public class DeviceController {
      @GetMapping({"/a/{id}", "/b/{id}"})
      public WVPResult aliases() { return null; }
    }
  `

  const routes = audit.extractJavaControllerRoutesFromSource(source, 'DeviceController.java')

  assert.deepEqual(routes.map((route) => `${route.method} ${route.path}`), [
    'GET /api/device/a/{id}',
    'GET /api/device/b/{id}',
  ])
})

test('extractJavaControllerRoutesFromSource emits one route per named RequestMapping path with path variables', () => {
  const source = `
    @RequestMapping(path = "/api/play")
    public class PlayController {
      @RequestMapping(value = {"/a/{id}", "/b/{id}"}, method = RequestMethod.GET)
      public WVPResult aliases() { return null; }
    }
  `

  const routes = audit.extractJavaControllerRoutesFromSource(source, 'PlayController.java')

  assert.deepEqual(routes.map((route) => `${route.method} ${route.path}`), [
    'GET /api/play/a/{id}',
    'GET /api/play/b/{id}',
  ])
})

test('extractJavaControllerRoutesFromSource skips non-route annotations between mapping and method', () => {
  const source = `
    @RequestMapping("/api/server")
    public class MediaServerController {
      @GetMapping(value = "/media_server/list")
      @ResponseBody
      @Operation(summary = "List media servers")
      public Object list() { return null; }
    }
  `

  const routes = audit.extractJavaControllerRoutesFromSource(source, 'MediaServerController.java')

  assert.deepEqual(routes.map((route) => `${route.method} ${route.path}`), [
    'GET /api/server/media_server/list',
  ])
})

test('extractJavaControllerRoutesFromSource skips non-route annotations with nested annotation arguments', () => {
  const source = `
    @RequestMapping("/api/server")
    public class ServerController {
      @GetMapping(value = "/media_server/list")
      @Operation(summary = "List media servers", responses = @ApiResponse(responseCode = "200"))
      public Object list() { return null; }
    }
  `

  const routes = audit.extractJavaControllerRoutesFromSource(source, 'ServerController.java')

  assert.deepEqual(routes.map((route) => `${route.method} ${route.path}`), [
    'GET /api/server/media_server/list',
  ])
})

test('extractJavaControllerRoutesFromSource treats bare method RequestMapping as ANY', () => {
  const source = `
    @RequestMapping("/api/user")
    public class UserController {
      @RequestMapping("/changePushKey")
      public WVPResult changePushKey() { return null; }
    }
  `

  const routes = audit.extractJavaControllerRoutesFromSource(source, 'UserController.java')

  assert.deepEqual(routes.map((route) => `${route.method} ${route.path}`), [
    'ANY /api/user/changePushKey',
  ])
})

test('extractRustRouterRoutesFromSource reads chained Axum route declarations', () => {
  const source = `
    Router::new()
      .route("/api/user/userInfo", get(user::user_info).post(user::user_info))
      .route("/api/play/start/:device_id/:channel_id", get(play::play_start))
      .route("/api/user/delete", delete(user::delete_user))
  `

  const routes = audit.extractRustRouterRoutesFromSource(source, 'src/router.rs')

  assert.deepEqual(routes.map((route) => `${route.method} ${route.path}`), [
    'DELETE /api/user/delete',
    'GET /api/play/start/{device_id}/{channel_id}',
    'GET /api/user/userInfo',
    'POST /api/user/userInfo',
  ])
})

test('extractFrontendApiCallsFromSource reads request URL and method from Vue API modules', () => {
  const source = `
    import request from '@/utils/request'

    export function playStart(deviceId, channelId) {
      return request({
        url: '/api/play/start/' + deviceId + '/' + channelId,
        method: 'get'
      })
    }

    export function addUser(data) {
      return request({ url: '/api/user/add', method: 'post', data })
    }
  `

  const calls = audit.extractFrontendApiCallsFromSource(source, 'web/src/api/play.js')

  assert.deepEqual(calls.map((call) => `${call.method} ${call.path}`), [
    'GET /api/play/start/{dynamic}/{dynamic}',
    'POST /api/user/add',
  ])
})

test('extractFrontendApiCallsFromSource preserves template literal dynamic URL segments', () => {
  const source = [
    'export function setSpeed(deviceId, channelDeviceId) {',
    '  return request({',
    '    url: `/api/front-end/scan/set/speed/${deviceId}/${channelDeviceId}`,',
    '    method: \'get\'',
    '  })',
    '}',
  ].join('\n')

  const calls = audit.extractFrontendApiCallsFromSource(source, 'web/src/api/front-end.js')

  assert.deepEqual(calls.map((call) => `${call.method} ${call.path}`), [
    'GET /api/front-end/scan/set/speed/{dynamic}/{dynamic}',
  ])
})

test('extractFrontendApiCallsFromSource ignores concatenation inside template literal interpolations', () => {
  const source = [
    'export function clearAlarm(qs) {',
    '  return request({',
    '    url: `/api/alarm/clear${qs ? \'?\' + qs : \'\'}`,',
    '    method: \'delete\'',
    '  })',
    '}',
  ].join('\n')

  const calls = audit.extractFrontendApiCallsFromSource(source, 'web/src/api/alarm.js')

  assert.deepEqual(calls.map((call) => `${call.method} ${call.path}`), [
    'DELETE /api/alarm/clear',
  ])
})

test('extractFrontendApiCallsFromSource strips template literal query strings from URL paths', () => {
  const source = [
    'export function deleteUser(id) {',
    '  return request({',
    '    url: `/api/user/delete?id=${id}`,',
    '    method: \'delete\'',
    '  })',
    '}',
  ].join('\n')

  const calls = audit.extractFrontendApiCallsFromSource(source, 'web/src/api/user.js')

  assert.deepEqual(calls.map((call) => `${call.method} ${call.path}`), [
    'DELETE /api/user/delete',
  ])
})

test('extractFrontendApiCallsFromSource strips concatenated query strings from URL paths', () => {
  const source = [
    'export function queryRecord(deviceId, channelId, startTime) {',
    '  return request({',
    "    url: '/api/gb_record/query/' + deviceId + '/' + channelId + '?startTime=' + startTime,",
    '    method: \'get\'',
    '  })',
    '}',
  ].join('\n')

  const calls = audit.extractFrontendApiCallsFromSource(source, 'web/src/api/record.js')

  assert.deepEqual(calls.map((call) => `${call.method} ${call.path}`), [
    'GET /api/gb_record/query/{dynamic}/{dynamic}',
  ])
})

test('extractVueRouterPagesFromSource reads route path, name, and component', () => {
  const source = `
    export const constantRoutes = [
      {
        path: '/device',
        name: 'Device',
        component: () => import('@/views/device/index'),
        meta: { title: '国标设备' }
      },
      {
        hidden: true,
        path: '/device/record/:deviceId/:channelDeviceId',
        name: 'DeviceRecord',
        component: () => import('@/views/device/channel/record')
      }
    ]
  `

  const pages = audit.extractVueRouterPagesFromSource(source, 'web/src/router/index.js')

  assert.deepEqual(pages.map((page) => `${page.name} ${page.path} ${page.component}`), [
    'Device /device @/views/device/index',
    'DeviceRecord /device/record/{deviceId}/{channelDeviceId} @/views/device/channel/record',
  ])
})

test('extractVueRouterPagesFromSource associates nested lazy components with their own route object', () => {
  const source = `
    export const asyncRoutes = [
      {
        path: '/device',
        name: '设备接入',
        component: Layout,
        children: [
          {
            path: '/device',
            name: 'Device',
            component: () => import('@/views/device/index')
          }
        ]
      }
    ]
  `

  const pages = audit.extractVueRouterPagesFromSource(source, 'web/src/router/index.js')

  assert.deepEqual(pages.map((page) => `${page.name} ${page.path} ${page.component}`), [
    'Device /device @/views/device/index',
  ])
})

test('extractVueRouterPagesFromSource resolves empty child route path to parent path', () => {
  const source = `
    export const asyncRoutes = [
      {
        path: '/live',
        name: 'LiveParent',
        component: Layout,
        children: [
          {
            path: '',
            name: 'Live',
            component: () => import('@/views/live/index')
          }
        ]
      }
    ]
  `

  const pages = audit.extractVueRouterPagesFromSource(source, 'web/src/router/index.js')

  assert.deepEqual(pages.map((page) => `${page.name} ${page.path} ${page.component}`), [
    'Live /live @/views/live/index',
  ])
})

test('extractVueRouterPagesFromSource joins relative child route path with parent path', () => {
  const source = `
    export const asyncRoutes = [
      {
        path: '/commonChannel',
        name: 'CommonChannel',
        component: Layout,
        children: [
          {
            path: 'region',
            name: 'Region',
            component: () => import('@/views/region/index')
          }
        ]
      }
    ]
  `

  const pages = audit.extractVueRouterPagesFromSource(source, 'web/src/router/index.js')

  assert.deepEqual(pages.map((page) => `${page.name} ${page.path} ${page.component}`), [
    'Region /commonChannel/region @/views/region/index',
  ])
})

test('compareRouteSets classifies aligned, missing, extra, and method mismatch routes', () => {
  const reference = [
    { method: 'GET', path: '/api/play/start/{deviceId}/{channelId}', source: 'PlayController.java' },
    { method: 'DELETE', path: '/api/user/delete', source: 'UserController.java' },
    { method: 'POST', path: '/api/platform/add', source: 'PlatformController.java' },
  ]
  const target = [
    { method: 'GET', path: '/api/play/start/{device_id}/{channel_id}', source: 'src/router.rs' },
    { method: 'GET', path: '/api/user/delete', source: 'src/router.rs' },
    { method: 'GET', path: '/api/local-only', source: 'src/router.rs' },
  ]

  const result = audit.compareRouteSets(reference, target)

  assert.deepEqual(result.aligned.map((item) => item.path), ['/api/play/start/{param}/{param}'])
  assert.deepEqual(result.missing.map((item) => `${item.method} ${item.path}`), ['POST /api/platform/add'])
  assert.deepEqual(result.extra.map((item) => `${item.method} ${item.path}`), ['GET /api/local-only'])
  assert.deepEqual(result.methodMismatch.map((item) => item.path), ['/api/user/delete'])
})


test('compareRouteSets treats reference ANY as wildcard for concrete target methods', () => {
  const reference = [
    { method: 'ANY', path: '/api/user/changePushKey', source: 'UserController.java' },
  ]
  const target = [
    { method: 'GET', path: '/api/user/changePushKey', source: 'src/router.rs' },
    { method: 'POST', path: '/api/user/changePushKey', source: 'src/router.rs' },
  ]

  const result = audit.compareRouteSets(reference, target)

  assert.deepEqual(result.aligned.map((item) => `${item.method} ${item.path}`), ['ANY /api/user/changePushKey'])
  assert.deepEqual(result.missing, [])
  assert.deepEqual(result.extra, [])
  assert.deepEqual(result.methodMismatch, [])
})

test('compareRouteSets emits one method mismatch per path', () => {
  const reference = [
    { method: 'GET', path: '/api/user/profile', source: 'UserController.java' },
    { method: 'POST', path: '/api/user/profile', source: 'UserController.java' },
    { method: 'PUT', path: '/api/user/profile', source: 'UserController.java' },
  ]
  const target = [
    { method: 'DELETE', path: '/api/user/profile', source: 'src/router.rs' },
  ]

  const result = audit.compareRouteSets(reference, target)

  assert.equal(result.methodMismatch.length, 1)
  assert.deepEqual(result.methodMismatch.map((item) => item.path), ['/api/user/profile'])
  assert.deepEqual(result.methodMismatch[0].referenceMethods, ['GET', 'POST', 'PUT'])
  assert.deepEqual(result.methodMismatch[0].targetMethods, ['DELETE'])
})

test('compareRouteSets reports extra target methods on paths with aligned methods', () => {
  const reference = [
    { method: 'GET', path: '/api/user/delete', source: 'UserController.java' },
  ]
  const target = [
    { method: 'GET', path: '/api/user/delete', source: 'src/router.rs' },
    { method: 'DELETE', path: '/api/user/delete', source: 'src/router.rs' },
  ]

  const result = audit.compareRouteSets(reference, target)

  assert.deepEqual(result.aligned.map((item) => `${item.method} ${item.path}`), ['GET /api/user/delete'])
  assert.deepEqual(result.extra.map((item) => `${item.method} ${item.path}`), ['DELETE /api/user/delete'])
  assert.deepEqual(result.methodMismatch, [])
})


const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')

test('buildAudit scans upstream backend/frontend and local backend/frontend trees', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'interface-coverage-'))
  const upstream = path.join(root, 'upstream')
  const local = path.join(root, 'local')

  fs.mkdirSync(path.join(upstream, 'src/main/java/com/example'), { recursive: true })
  fs.mkdirSync(path.join(upstream, 'web/src/api'), { recursive: true })
  fs.mkdirSync(path.join(upstream, 'web/src/router'), { recursive: true })
  fs.mkdirSync(path.join(local, 'src'), { recursive: true })
  fs.mkdirSync(path.join(local, 'web/src/api'), { recursive: true })
  fs.mkdirSync(path.join(local, 'web/src/router'), { recursive: true })

  fs.writeFileSync(path.join(upstream, 'src/main/java/com/example/PlayController.java'), `
    @RequestMapping("/api/play")
    public class PlayController {
      @GetMapping("/start/{deviceId}/{channelId}") public Object start() { return null; }
    }
  `)
  fs.writeFileSync(path.join(upstream, 'web/src/api/play.js'), `
    export function start(deviceId, channelId) { return request({ url: '/api/play/start/' + deviceId + '/' + channelId, method: 'get' }) }
  `)
  fs.writeFileSync(path.join(upstream, 'web/src/router/index.js'), `
    export const constantRoutes = [{ path: '/live', name: 'Live', component: () => import('@/views/live/index') }]
  `)
  fs.writeFileSync(path.join(local, 'src/router.rs'), `
    Router::new().route("/api/play/start/:device_id/:channel_id", get(play::play_start))
  `)
  fs.writeFileSync(path.join(local, 'web/src/api/play.js'), `
    export function start(deviceId, channelId) { return request({ url: '/api/play/start/' + deviceId + '/' + channelId, method: 'get' }) }
  `)
  fs.writeFileSync(path.join(local, 'web/src/router/index.js'), `
    export const constantRoutes = [{ path: '/live', name: 'Live', component: () => import('@/views/live/index') }]
  `)

  const result = audit.buildAudit({ upstream, local, commit: 'test123' })

  assert.equal(result.baseline.commit, 'test123')
  assert.equal(result.javaRoutes.length, 1)
  assert.equal(result.rustRoutes.length, 1)
  assert.equal(result.upstreamFrontendApi.length, 1)
  assert.equal(result.localFrontendApi.length, 1)
  assert.equal(result.upstreamPages.length, 1)
  assert.equal(result.localPages.length, 1)
  assert.equal(result.comparisons.backendRoutes.aligned.length, 1)
  assert.equal(result.comparisons.upstreamFrontendToRust.aligned.length, 1)
})

test('buildMarkdownReport includes baseline, counts, and top gaps', () => {
  const markdown = audit.buildMarkdownReport({
    baseline: { upstream: '/tmp/reference-java-impl', local: '/repo', commit: 'b760458' },
    generatedAt: '2026-05-30T00:00:00.000Z',
    counts: { javaRoutes: 2, rustRoutes: 1, upstreamFrontendApi: 1, localFrontendApi: 1, upstreamPages: 1, localPages: 1 },
    comparisons: {
      backendRoutes: {
        aligned: [{ method: 'GET', path: '/api/play/start/{param}/{param}' }],
        missing: [{ method: 'POST', path: '/api/platform/add', source: 'PlatformController.java' }],
        extra: [{ method: 'DELETE', path: '/api/local-only', source: 'src/router.rs' }],
        methodMismatch: [],
      },
      upstreamFrontendToRust: { aligned: [], missing: [], extra: [], methodMismatch: [] },
      upstreamFrontendToLocalFrontend: { aligned: [], missing: [], extra: [], methodMismatch: [] },
      upstreamPagesToLocalPages: { aligned: [], missing: [], extra: [], methodMismatch: [] },
    },
  })

  assert.match(markdown, /# GBServer Interface Coverage Report (Phase 0)/)
  assert.match(markdown, /Baseline commit: `b760458`/)
  assert.match(markdown, /Java controller routes: 2/)
  assert.match(markdown, /POST `\/api\/platform\/add`/)
  assert.match(markdown, /#### Extra target entries/)
  assert.match(markdown, /\| DELETE \| `\/api\/local-only` \| src\/router\.rs \|/)
})
