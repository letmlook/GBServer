import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface DeviceQueryParams {
  page?: number
  count?: number
  query?: string
  status?: string
}

export function queryDevices(params: DeviceQueryParams) {
  return request<WvpResult<{ total: number; list: DeviceRecord[] }>>({
    method: 'get',
    url: '/device/query/devices',
    params
  })
}

export function queryDeviceOne(deviceId: string) {
  return request<WvpResult<DeviceRecord>>({
    method: 'get',
    url: `/device/query/devices/${deviceId}`
  })
}

export function deleteDevice(deviceId: string) {
  return request<WvpResult>({
    method: 'delete',
    url: `/device/query/devices/${deviceId}/delete`
  })
}

export function sync(deviceId: string) {
  return request<WvpResult>({
    method: 'get',
    url: `/device/query/devices/${deviceId}/sync`
  })
}

export function syncStatus(deviceId: string) {
  return request<WvpResult<{ total: number; current: number; errorMsg?: string }>>({
    method: 'get',
    url: '/device/query/sync_status',
    params: { deviceId }
  })
}

export function updateDeviceTransport(deviceId: string, streamMode: string) {
  return request<WvpResult>({
    method: 'post',
    url: `/device/query/transport/${deviceId}/${streamMode}`
  })
}

export function setGuard(deviceId: string) {
  return request<WvpResult>({
    method: 'get',
    url: '/device/control/guard',
    params: { deviceId, guardCmd: 'SetGuard' }
  })
}

export function resetGuard(deviceId: string) {
  return request<WvpResult>({
    method: 'get',
    url: '/device/control/guard',
    params: { deviceId, guardCmd: 'ResetGuard' }
  })
}

export function subscribeCatalog(params: { id: string; cycle?: number }) {
  return request<WvpResult>({
    method: 'get',
    url: '/device/query/subscribe/catalog',
    params
  })
}

export function subscribeMobilePosition(params: { id: string; cycle?: number; interval?: number }) {
  return request<WvpResult>({
    method: 'get',
    url: '/device/query/subscribe/mobile-position',
    params
  })
}

export function queryBasicParam(deviceId: string) {
  return request<WvpResult>({
    method: 'get',
    url: `/device/config/query/${deviceId}/BasicParam`
  })
}

export function add(data: Partial<DeviceRecord>) {
  return request<WvpResult>({
    method: 'post',
    url: '/device/query/device/add',
    data
  })
}

export function update(data: Partial<DeviceRecord>) {
  return request<WvpResult>({
    method: 'post',
    url: '/device/query/device/update',
    data
  })
}

export function queryChannels(deviceId: string, params: DeviceQueryParams & { online?: boolean; channelType?: number }) {
  return request<WvpResult<{ total: number; list: ChannelRecord[] }>>({
    method: 'get',
    url: `/device/query/devices/${deviceId}/channels`,
    params
  })
}

export function queryChannelTree(deviceId: string, params: DeviceQueryParams & { parentId?: string; onlyCatalog?: boolean }) {
  return request<WvpResult<{ total: number; list: ChannelRecord[] }>>({
    method: 'get',
    url: `/device/query/tree/${deviceId}`,
    params
  })
}

export function queryDeviceTree(deviceId: string, params: DeviceQueryParams & { parentId?: string; onlyCatalog?: boolean }) {
  return request<WvpResult<{ total: number; list: ChannelRecord[] }>>({
    method: 'get',
    url: `/device/query/tree/${deviceId}`,
    params
  })
}

export function deviceRecord(params: { deviceId: string; channelId: string; recordCmdStr: string }) {
  return request<WvpResult>({
    method: 'get',
    url: '/device/control/record',
    params
  })
}

export interface DeviceRecord {
  id?: number
  deviceId: string
  name?: string
  manufacturer?: string
  model?: string
  firmware?: string
  transport?: string
  streamMode?: string
  ip?: string
  port?: number
  expires?: number
  heartBeatInterval?: number
  heartBeatCount?: number
  registerTime?: string
  updateTime?: string
  createTime?: string
  online?: number | boolean
  channelCount?: number
  mediaServerId?: string
  sdpIp?: string
  status?: string
  gbId?: string
  gbDeviceId?: string
  treePath?: string
}

export interface ChannelRecord {
  id?: number
  deviceId: string
  channelId: string
  name?: string
  manufacturer?: string
  model?: string
  owner?: string
  civilCode?: string
  address?: string
  status?: string
  parental?: string
  longitude?: number
  latitude?: number
  streamIdentification?: string
  channelType?: number
  hasAudio?: boolean
  subCount?: number
  parentId?: string
  audio?: boolean
  registerStatus?: string
}
