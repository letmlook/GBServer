import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export function getMediaServerList() {
  return request<WvpResult<MediaServer[]>>({
    method: 'get',
    url: '/server/media_server/list'
  })
}

export function getMediaServerOnlineList() {
  return request<WvpResult<MediaServer[]>>({
    method: 'get',
    url: '/server/media_server/online/list'
  })
}

export function getMediaServerOne(id: string) {
  return request<WvpResult<MediaServer>>({
    method: 'get',
    url: `/server/media_server/one/${id}`
  })
}

export function saveMediaServer(data: Partial<MediaServer>) {
  return request<WvpResult>({
    method: 'post',
    url: '/server/media_server/save',
    data
  })
}

export function deleteMediaServer(id: string) {
  return request<WvpResult>({
    method: 'delete',
    url: '/server/media_server/delete',
    params: { id }
  })
}

export function checkMediaServer(id: string) {
  return request<WvpResult<{ code: number; msg: string }>>({
    method: 'get',
    url: '/server/media_server/check',
    params: { id }
  })
}

export function getMediaLoad(id: string) {
  return request<WvpResult<{ load: number }>>({
    method: 'get',
    url: '/server/media_server/load',
    params: { id }
  })
}

export function getMediaInfo(id: string) {
  return request<WvpResult<{ mediaServerId: string; mediaList: MediaInfo[] }>>({
    method: 'get',
    url: '/server/media_server/media_info',
    params: { id }
  })
}

export interface MediaServer {
  id?: string
  ip: string
  httpPort: number
  rtspPort?: number
  rtmpPort?: number
  secret: string
  enabled?: boolean
  hookAliveInterval?: number
  status?: boolean
  type?: string
  streamMode?: string
  createTime?: string
  updateTime?: string
  lastKeepaliveTime?: string
  lastRegisterTime?: string
}

export interface MediaInfo {
  app: string
  stream: string
  schema?: string
  readerCount?: number
  totalReaderCount?: number
  originType?: number
  originUrl?: string
  bytesSpeed?: number
}
