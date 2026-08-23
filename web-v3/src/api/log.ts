import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface LogRecord {
  id?: number
  time?: string
  level?: string
  logger?: string
  thread?: string
  message?: string
  source?: string
  tone?: 'info' | 'warn' | 'error' | 'debug'
}

export interface LogQueryParams {
  page?: number
  count?: number
  query?: string
  startTime?: string
  endTime?: string
  level?: string
}

export function getLogList(params: LogQueryParams) {
  return request<WvpResult<{ total: number; list: LogRecord[] }>>({
    method: 'get',
    url: '/log/list',
    params
  })
}

export function getLogFile(fileName: string) {
  return request<Blob>({
    method: 'get',
    url: `/log/file/${encodeURIComponent(fileName)}`,
    responseType: 'blob'
  })
}

export interface SystemInfo {
  cpu?: number
  /** 后端 summary 字段：CPU 当前使用百分比 */
  cpu_usage?: number
  /** 后端 summary 字段：内存当前使用百分比 */
  mem_usage?: number
  memory?: {
    total?: number
    used?: number
    free?: number
    /** 后端返回的内存历史采样数组 */
    mem?: { data: number; time: string }[]
  }
  disk?: { total: number; used: number; free: number; path: string }[]
  disk_usage?: number
  network?: { name: string; rx: number; tx: number }[]
  netTotal?: number
  uptime?: number
  version?: string
  buildTime?: string
  mediaServerCount?: number
  deviceOnline?: number
  deviceTotal?: number
  channelOnline?: number
  channelTotal?: number
}

export function getSystemInfo() {
  return request<WvpResult<SystemInfo>>({
    method: 'get',
    url: '/server/system/info'
  })
}

export function getSystemConfigInfo() {
  return request<WvpResult<Record<string, unknown>>>({
    method: 'get',
    url: '/server/system/configInfo'
  })
}

export function getResourceInfo() {
  return request<WvpResult<Record<string, unknown>>>({
    method: 'get',
    url: '/server/resource/info'
  })
}

export function getServerInfo() {
  return request<WvpResult<Record<string, unknown>>>({
    method: 'get',
    url: '/server/info'
  })
}
