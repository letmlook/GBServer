import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface JtTerminal {
  id?: number
  phoneNumber: string
  terminalId?: string
  provinceId?: string
  provinceText?: string
  cityId?: string
  cityText?: string
  makerId?: string
  model?: string
  plateColor?: number | string
  plateNo?: string
  longitude?: number
  latitude?: number
  /** 后端（与 WVP `JTDevice.status`）是**布尔**；早期声明成 number，
   *  页面用 `row.status === 1` 比较 → 在线终端也显示"离线"。 */
  status?: boolean
  mediaServerId?: string
  sdpIp?: string
  authCode?: string
  registerTime?: string
  updateTime?: string
  createTime?: string
}

export function getJtTerminalList(params: { page?: number; count?: number; query?: string }) {
  return request<WvpResult<{ total: number; list: JtTerminal[] }>>({
    method: 'get',
    url: '/jt1078/terminal/list',
    params
  })
}

export function getJtTerminalOne(id: number | string) {
  return request<WvpResult<JtTerminal>>({
    method: 'get',
    url: '/jt1078/terminal/one',
    params: { id }
  })
}

export function addJtTerminal(data: Partial<JtTerminal>) {
  return request<WvpResult>({
    method: 'post',
    url: '/jt1078/terminal/add',
    data
  })
}

export function updateJtTerminal(data: Partial<JtTerminal>) {
  return request<WvpResult>({
    method: 'post',
    url: '/jt1078/terminal/update',
    data
  })
}

/**
 * 删除终端（WVP 契约：`DELETE /api/jt1078/terminal/delete`）。
 *
 * 早期用 GET → 后端只注册 DELETE，必然 405；参数名也应为 `phoneNumber`
 * （后端同时兼容数据库主键 `id`，JT 设备页传的就是它）。
 */
export function deleteJtTerminal(idOrPhone: number | string) {
  const isNumericId = typeof idOrPhone === 'number' || /^\d{1,10}$/.test(String(idOrPhone))
  return request<WvpResult>({
    method: 'delete',
    url: '/jt1078/terminal/delete',
    params: isNumericId ? { id: idOrPhone } : { phoneNumber: idOrPhone }
  })
}

export interface JtChannel {
  id?: number
  terminalDbId?: number
  phoneNumber?: string
  channelId: number
  /** 后端（与 WVP `JTChannel.name`）的字段名是 `name`；`channelName` 是兼容别名 */
  name?: string
  channelName?: string
  hasAudio?: boolean
  hasVideo?: boolean
  ptzType?: number
  status?: boolean
}

export function getJtChannelList(terminalDbId: number | string) {
  return request<WvpResult<{ total: number; list: JtChannel[] }>>({
    method: 'get',
    url: '/jt1078/terminal/channel/list',
    params: { terminalDbId }
  })
}

export function addJtChannel(data: Partial<JtChannel>) {
  return request<WvpResult>({
    method: 'post',
    url: '/jt1078/terminal/channel/add',
    data
  })
}

export function updateJtChannel(data: Partial<JtChannel>) {
  return request<WvpResult>({
    method: 'post',
    url: '/jt1078/terminal/channel/update',
    data
  })
}

export function deleteJtChannel(id: number | string) {
  return request<WvpResult>({
    method: 'delete',
    url: `/jt1078/terminal/channel/delete/${id}`
  })
}

export interface JtArea {
  id?: number
  phoneNumber: string
  label?: string
  shape?: 'circle' | 'polygon' | 'rectangle'
  centerLat?: number
  centerLon?: number
  radiusM?: number
  ltLat?: number
  ltLon?: number
  rbLat?: number
  rbLon?: number
  pointsJson?: string
  createTime?: string
  updateTime?: string
}

export function getJtAreaCircleList(phone: string) {
  return request<WvpResult<{ count: number; items: JtArea[] }>>({
    method: 'get',
    url: '/jt1078/area/circle/query',
    params: { phone }
  })
}

export function addJtAreaCircle(data: Partial<JtArea>) {
  return request<WvpResult>({
    method: 'post',
    url: '/jt1078/area/circle/add',
    data
  })
}

export function deleteJtAreaCircle(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/jt1078/area/circle/delete',
    params: { id }
  })
}

export function getJtAreaPolygonList(phone: string) {
  return request<WvpResult<{ count: number; items: JtArea[] }>>({
    method: 'get',
    url: '/jt1078/area/polygon/query',
    params: { phone }
  })
}

export function setJtAreaPolygon(data: Partial<JtArea>) {
  return request<WvpResult>({
    method: 'post',
    url: '/jt1078/area/polygon/set',
    data
  })
}

export function deleteJtAreaPolygon(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/jt1078/area/polygon/delete',
    params: { id }
  })
}

export function getJtAreaRectangleList(phone: string) {
  return request<WvpResult<{ count: number; items: JtArea[] }>>({
    method: 'get',
    url: '/jt1078/area/rectangle/query',
    params: { phone }
  })
}

export function addJtAreaRectangle(data: Partial<JtArea>) {
  return request<WvpResult>({
    method: 'post',
    url: '/jt1078/area/rectangle/add',
    data
  })
}

export function deleteJtAreaRectangle(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/jt1078/area/rectangle/delete',
    params: { id }
  })
}

export interface JtRoute {
  id?: number
  phoneNumber: string
  label?: string
  waypointsJson?: string
  createTime?: string
  updateTime?: string
}

export function getJtRouteList(phone: string) {
  return request<WvpResult<{ count: number; items: JtRoute[] }>>({
    method: 'get',
    url: '/jt1078/route/query',
    params: { phone }
  })
}

export function setJtRoute(data: Partial<JtRoute>) {
  return request<WvpResult>({
    method: 'post',
    url: '/jt1078/route/set',
    data
  })
}

export function deleteJtRoute(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/jt1078/route/delete',
    params: { id }
  })
}
