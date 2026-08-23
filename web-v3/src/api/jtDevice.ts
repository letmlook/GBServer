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
  plateColor?: number
  plateNo?: string
  longitude?: number
  latitude?: number
  status?: number
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

export function deleteJtTerminal(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/jt1078/terminal/delete',
    params: { id }
  })
}

export interface JtChannel {
  id?: number
  terminalDbId?: number
  phoneNumber?: string
  channelId: number
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
