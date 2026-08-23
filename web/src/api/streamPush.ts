import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface StreamPush {
  id?: number
  app: string
  stream: string
  gbId?: string
  status?: number
  url?: string
  mediaServerId?: string
  createTime?: string
  updateTime?: string
}

export function getStreamPushList(params: { page?: number; count?: number; query?: string }) {
  return request<WvpResult<{ total: number; list: StreamPush[] }>>({
    method: 'get',
    url: '/push/list',
    params
  })
}

export function addStreamPush(data: Partial<StreamPush>) {
  return request<WvpResult>({
    method: 'post',
    url: '/push/add',
    data
  })
}

export function updateStreamPush(data: Partial<StreamPush>) {
  return request<WvpResult>({
    method: 'post',
    url: '/push/update',
    data
  })
}

export function deleteStreamPush(id: number | string) {
  return request<WvpResult>({
    method: 'delete',
    url: '/push/remove',
    params: { id }
  })
}

export function batchDeleteStreamPush(ids: (number | string)[]) {
  return request<WvpResult>({
    method: 'delete',
    url: '/push/batchRemove',
    params: { ids: ids.join(',') }
  })
}

export function startStreamPush(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/push/start',
    params: { id }
  })
}

export function stopStreamPush(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/push/stop',
    params: { id }
  })
}

export function uploadStreamPush(data: { id: number | string; url: string }) {
  return request<WvpResult>({
    method: 'post',
    url: '/push/upload',
    data
  })
}

export function saveToGb(id: number | string) {
  return request<WvpResult>({
    method: 'post',
    url: '/push/save_to_gb',
    params: { id }
  })
}

export function removeFromGb(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/push/remove_form_gb',
    params: { id }
  })
}

export function forceClose(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/push/forceClose',
    params: { id }
  })
}
