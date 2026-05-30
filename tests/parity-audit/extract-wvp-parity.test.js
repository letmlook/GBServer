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
    { method: 'GET', path: '/api/play/start/{deviceId}/{channelId}', source: 'PlayController.java' },
    { method: 'POST', path: '/api/play/stop/{deviceId}/{channelId}', source: 'PlayController.java' },
    { method: 'GET', path: '/api/play/ssrc', source: 'PlayController.java' },
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
