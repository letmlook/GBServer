const assert = require('node:assert/strict')
const test = require('node:test')

const audit = require('../../scripts/parity-audit/extract-wvp-parity')

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
