import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface PlatformQueryParams {
  page?: number
  count?: number
  query?: string
}

export interface Platform {
  id?: number
  serverGbId: string
  serverIp?: string
  serverPort?: number
  name?: string
  username?: string
  password?: string
  realm?: string
  transport?: string
  registerInterval?: number
  heartBeatInterval?: number
  heartBeatCount?: number
  expires?: number
  enable?: boolean
  status?: boolean
  createTime?: string
  updateTime?: string
}

export function getPlatformList(params: PlatformQueryParams) {
  return request<WvpResult<{ total: number; list: Platform[] }>>({
    method: 'get',
    url: '/platform/query',
    params
  })
}

export function getPlatformOne(id: number | string) {
  return request<WvpResult<Platform>>({
    method: 'get',
    url: `/platform/info/${id}`
  })
}

export function addPlatform(data: Partial<Platform>) {
  return request<WvpResult>({
    method: 'post',
    url: '/platform/add',
    data
  })
}

export function updatePlatform(data: Partial<Platform>) {
  return request<WvpResult>({
    method: 'post',
    url: '/platform/update',
    data
  })
}

export function deletePlatform(id: number | string) {
  return request<WvpResult>({
    method: 'delete',
    url: '/platform/delete',
    params: { id }
  })
}

export function platformExit(deviceGbId: string) {
  return request<WvpResult>({
    method: 'get',
    url: `/platform/exit/${deviceGbId}`
  })
}

export function getPlatformServerConfig() {
  return request<WvpResult<{ ip: string; port: number; id: string; realm: string }>>({
    method: 'get',
    url: '/platform/server_config'
  })
}

export function addPlatformCatalog(data: { platformId: number | string; name: string; parentId?: number | string; civilCode?: string; businessGroup?: string }) {
  return request<WvpResult>({
    method: 'post',
    url: '/platform/catalog/add',
    data
  })
}

export function editPlatformCatalog(data: { id: number | string; name: string; parentId?: number | string; civilCode?: string; businessGroup?: string }) {
  return request<WvpResult>({
    method: 'post',
    url: '/platform/catalog/edit',
    data
  })
}
