import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface CloudRecord {
  id?: number
  app?: string
  stream?: string
  callId?: string
  mediaServerId?: string
  gbId?: string
  startTime?: string
  endTime?: string
  filePath?: string
  folder?: string
  size?: number
  createTime?: string
}

export interface CloudRecordListParams {
  page?: number
  count?: number
  query?: string
  app?: string
  stream?: string
  startTime?: string
  endTime?: string
  deviceId?: string
  channelId?: string
}

export function getCloudRecordList(params: CloudRecordListParams) {
  return request<WvpResult<{ total: number; list: CloudRecord[] }>>({
    method: 'get',
    url: '/cloud/record/list',
    params
  })
}

export function getCloudRecordListUrl(params: CloudRecordListParams) {
  return request<WvpResult<{ total: number; list: CloudRecord[] }>>({
    method: 'get',
    url: '/cloud/record/list-url',
    params
  })
}

export function getCloudRecordDateList(params: { deviceId?: string; channelId?: string; app?: string; stream?: string }) {
  return request<WvpResult<{ list: { date: string; count: number }[] }>>({
    method: 'get',
    url: '/cloud/record/date/list',
    params
  })
}

export function getCloudRecordPlayPath(id: number | string) {
  return request<WvpResult<{ path: string }>>({
    method: 'get',
    url: '/cloud/record/play/path',
    params: { id }
  })
}

export function getCloudRecordLoad(params: { id: number | string; startTime?: string; endTime?: string }) {
  return request<WvpResult<{ records: CloudRecord[] }>>({
    method: 'get',
    url: '/cloud/record/loadRecord',
    params
  })
}

export function seekCloudRecord(streamId: string, seekTime: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/cloud/record/seek',
    params: { streamId, seekTime }
  })
}

export function speedCloudRecord(streamId: string, speed: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/cloud/record/speed',
    params: { streamId, speed }
  })
}

export function deleteCloudRecord(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/cloud/record/delete',
    params: { id }
  })
}

export function addCloudRecordTask(data: Partial<CloudRecord>) {
  return request<WvpResult>({
    method: 'post',
    url: '/cloud/record/task/add',
    data
  })
}

export function getCloudRecordTaskList(params: { page?: number; count?: number }) {
  return request<WvpResult<{ total: number; list: CloudRecord[] }>>({
    method: 'get',
    url: '/cloud/record/task/list',
    params
  })
}

export function downloadCloudRecordZip(ids: (number | string)[]) {
  return request<WvpResult<{ url: string }>>({
    method: 'get',
    url: '/cloud/record/download/zip',
    params: { ids: ids.join(',') }
  })
}

export function getCloudRecordCollectList() {
  return request<WvpResult<{ list: any[] }>>({
    method: 'get',
    url: '/cloud/record/collect/list'
  })
}

export function addCloudRecordCollect(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/cloud/record/collect/add',
    params: { id }
  })
}

export function deleteCloudRecordCollect(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/cloud/record/collect/delete',
    params: { id }
  })
}
