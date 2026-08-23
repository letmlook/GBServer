import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface StreamProxyQueryParams {
  page?: number
  count?: number
  query?: string
}

export interface StreamProxy {
  id?: number
  name: string
  type?: string
  app?: string
  stream?: string
  url?: string
  destUrl?: string
  enabled?: boolean
  status?: number
  createTime?: string
  updateTime?: string
}

export function getStreamProxyList(params: StreamProxyQueryParams) {
  return request<WvpResult<{ total: number; list: StreamProxy[] }>>({
    method: 'get',
    url: '/proxy/list',
    params
  })
}

export function getStreamProxyOne(id: number | string) {
  return request<WvpResult<StreamProxy>>({
    method: 'get',
    url: '/proxy/one',
    params: { id }
  })
}

export function addStreamProxy(data: Partial<StreamProxy>) {
  return request<WvpResult>({
    method: 'post',
    url: '/proxy/add',
    data
  })
}

export function updateStreamProxy(data: Partial<StreamProxy>) {
  return request<WvpResult>({
    method: 'post',
    url: '/proxy/update',
    data
  })
}

export function saveStreamProxy(data: Partial<StreamProxy>) {
  return request<WvpResult>({
    method: 'post',
    url: '/proxy/save',
    data
  })
}

export function startStreamProxy(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/proxy/start',
    params: { id }
  })
}

export function stopStreamProxy(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/proxy/stop',
    params: { id }
  })
}

export function deleteStreamProxy(id: number | string) {
  return request<WvpResult>({
    method: 'delete',
    url: '/proxy/delete',
    params: { id }
  })
}
