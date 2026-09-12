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

/**
 * `GET /api/server/system/info`。
 *
 * 注意 `cpu` / `mem` / `net` 是**折线图的历史采样数组**（dashboard 的 v-charts
 * 直接 setData 覆盖渲染），当前的使用率是标量 `cpu_usage` / `mem_usage` /
 * `disk_usage`（0-100 的百分数）。
 * 此前类型把 `cpu` 声明成 `number`，系统信息页按 number 渲染 →
 * 卡片显示成 "[object Object]"，进度条 percentage 收到数组而失效。
 */
export interface SystemInfo {
  /** 历史采样：data 是 0-1 的小数 */
  cpu?: { time: string; data: number }[]
  mem?: { time: string; data: number }[]
  net?: { time: string; out: number; in: number }[]
  cpu_usage?: number
  mem_usage?: number
  disk_usage?: number
  memory?: {
    /** 字节 */
    total?: number
    used?: number
    free?: number
    mem?: { data: number; time: string }[]
  }
  /** 每个挂载点：`total`/`used` 是字节（系统信息页卡片），`use`/`free` 是 GB（dashboard 柱状图） */
  disk?: { path: string; total: number; used: number; use?: number; free?: number }[]
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
