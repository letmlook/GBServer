import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface StreamPush {
  id?: number
  app: string
  stream: string
  /** 后端（与 WVP `StreamPush.pushing/status`）是**布尔** */
  status?: boolean
  pushing?: boolean
  startOfflinePush?: boolean
  /** 后端按 app/stream/媒体节点算出的**推流地址**（不是用户填的源 URL） */
  pushUrl?: string
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

/**
 * 删除推流。WVP / 后端的这个端点是 **POST + query `id`**（不是 DELETE）；
 * 早期写成 DELETE → 405，推流记录删不掉。
 */
export function deleteStreamPush(id: number | string) {
  return request<WvpResult>({
    method: 'post',
    url: '/push/remove',
    params: { id }
  })
}

/** 批量删除：DELETE + **JSON body** `{ids:[...]}`（放 query 会被 Json 提取器拒成 415） */
export function batchDeleteStreamPush(ids: (number | string)[]) {
  return request<WvpResult>({
    method: 'delete',
    url: '/push/batchRemove',
    data: { ids }
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

/**
 * 上传推流文件：后端是 **multipart/form-data**，字段名 `file`（可选 `app`/`stream`）。
 * 早期发 JSON `{id,url}` → 被 Multipart 提取器拒成 400。
 */
export function uploadStreamPush(file: File, app?: string, stream?: string) {
  const form = new FormData()
  form.append('file', file)
  if (app) form.append('app', app)
  if (stream) form.append('stream', stream)
  return request<WvpResult>({
    method: 'post',
    url: '/push/upload',
    data: form,
    headers: { 'Content-Type': 'multipart/form-data' }
  })
}

/** 绑定国标：POST + JSON body `{id, deviceId, channelId}`（后端从 body 取） */
export function saveToGb(id: number | string, deviceId: string, channelId: string) {
  return request<WvpResult>({
    method: 'post',
    url: '/push/save_to_gb',
    data: { id, deviceId, channelId }
  })
}

/** 解绑国标：DELETE + JSON body `{id}`（后端从 body 取 id） */
export function removeFromGb(id: number | string) {
  return request<WvpResult>({
    method: 'delete',
    url: '/push/remove_form_gb',
    data: { id }
  })
}

export function forceClose(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/push/forceClose',
    params: { id }
  })
}
