import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

/**
 * 云端录像。
 *
 * `id` 是**数据库主键**（`gb_cloud_record.id`，来自 `on_record_mp4` 钩子落库），
 * 播放 / 删除 / 打包下载都用它；`recordId` 是历史的组合串
 * （`媒体节点::app::stream::文件名`），后端仍然兼容。
 */
export interface CloudRecord {
  id?: number
  /** 组合串 id（历史格式，后端兼容） */
  recordId?: string
  app?: string
  stream?: string
  /** 从流名 `{设备}_{通道}` 解析出的国标编号 */
  deviceId?: string
  channelId?: string
  callId?: string
  mediaServerId?: string
  /** 毫秒时间戳 */
  startTime?: number
  endTime?: number
  timeLen?: number
  fileName?: string
  filePath?: string
  folder?: string
  /** 字节数 */
  size?: number
  collect?: boolean
  /** 播放/下载地址（由后端拼好） */
  playPath?: string
  httpPath?: string
  downloadPath?: string
  createTime?: string
}

/** `/cloud/record/list-url` 返回的是"可下载文件"视图 */
export interface CloudRecordFile {
  id: number
  app: string
  stream: string
  fileName: string
  url: string
  startTime?: number
  endTime?: number
  duration?: number
  fileSize?: number
}

export interface CloudRecordListParams {
  page?: number
  count?: number
  query?: string
  app?: string
  stream?: string
  /** WVP 的时间格式：`yyyy-MM-dd HH:mm:ss`（后端也接受 ISO/毫秒） */
  startTime?: string
  endTime?: string
  deviceId?: string
  channelId?: string
  mediaServerId?: string
}

export function getCloudRecordList(params: CloudRecordListParams) {
  return request<WvpResult<{ total: number; list: CloudRecord[] }>>({
    method: 'get',
    url: '/cloud/record/list',
    params
  })
}

export function getCloudRecordListUrl(params: CloudRecordListParams) {
  return request<WvpResult<{ total: number; list: CloudRecordFile[] }>>({
    method: 'get',
    url: '/cloud/record/list-url',
    params
  })
}

/** 后端（与 WVP 一致）返回**裸字符串数组** `["2024-05-01", ...]` */
export function getCloudRecordDateList(params: {
  deviceId?: string
  channelId?: string
  app?: string
  stream?: string
}) {
  return request<WvpResult<string[]>>({
    method: 'get',
    url: '/cloud/record/date/list',
    params
  })
}

/** WVP 的参数名是 `recordId`（后端也接受 `id`） */
export function getCloudRecordPlayPath(recordId: number | string) {
  return request<
    WvpResult<{ playPath: string; httpPath: string; httpsPath: string; filePath?: string }>
  >({
    method: 'get',
    url: '/cloud/record/play/path',
    params: { recordId }
  })
}

export function getCloudRecordLoad(params: {
  recordId: number | string
  app?: string
  stream?: string
  startTime?: string
  endTime?: string
}) {
  return request<WvpResult<Record<string, unknown>>>({
    method: 'get',
    url: '/cloud/record/loadRecord',
    params
  })
}

export function seekCloudRecord(recordId: number | string, seekTime: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/cloud/record/seek',
    params: { recordId, seek: seekTime }
  })
}

export function speedCloudRecord(recordId: number | string, speed: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/cloud/record/speed',
    params: { recordId, speed }
  })
}

/**
 * 删除云端录像（WVP 契约：`DELETE /api/cloud/record/delete`，body `{ids:[...]}`）。
 *
 * 早期实现是 `GET /cloud/record/delete?id=...`：该 path 只注册了 DELETE → 405，
 * 而且后端从 **body** 读 `ids`，query 上的 `id` 根本进不去。
 */
export function deleteCloudRecord(ids: (number | string) | (number | string)[]) {
  return request<WvpResult<{ deleted: string[]; failed: string[] }>>({
    method: 'delete',
    url: '/cloud/record/delete',
    data: { ids: Array.isArray(ids) ? ids.map(String) : [String(ids)] }
  })
}

/**
 * 建云端录像任务。后端（与 WVP 一致）是 **GET**，参数走 query。
 * 早期写成 POST + JSON body：405，且参数也进不了 `Query`。
 */
export function addCloudRecordTask(params: {
  app?: string
  stream?: string
  mediaServerId?: string
  startTime?: string
  endTime?: string
}) {
  return request<WvpResult>({
    method: 'get',
    url: '/cloud/record/task/add',
    params
  })
}

export function getCloudRecordTaskList(params: { page?: number; count?: number }) {
  return request<WvpResult<{ total: number; list: CloudRecord[] }>>({
    method: 'get',
    url: '/cloud/record/task/list',
    params
  })
}

/**
 * 打包下载。后端按**数字主键**解析 `ids`（逗号分隔），
 * 所以必须传 `CloudRecord.id`（组合串 `recordId` 解析不出数字）。
 */
export function downloadCloudRecordZip(ids: (number | string)[]) {
  return request<WvpResult<{ url: string }>>({
    method: 'get',
    url: '/cloud/record/download/zip',
    params: { ids: ids.join(',') }
  })
}

export function getCloudRecordCollectList() {
  return request<WvpResult<{ total: number; list: Record<string, unknown>[] }>>({
    method: 'get',
    url: '/cloud/record/collect/list'
  })
}

/** 收藏：后端只认 `recordId`（组合串），传数字主键时后端会自动换算 */
export function addCloudRecordCollect(recordId: number | string, extra?: {
  deviceId?: string
  channelId?: string
  name?: string
}) {
  return request<WvpResult>({
    method: 'get',
    url: '/cloud/record/collect/add',
    params: { recordId, ...extra }
  })
}

export function deleteCloudRecordCollect(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/cloud/record/collect/delete',
    params: { id }
  })
}
