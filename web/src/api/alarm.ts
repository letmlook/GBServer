import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface AlarmQueryParams {
  page?: number
  count?: number
  query?: string
  startTime?: string
  endTime?: string
  level?: string
  alarmType?: string
}

export interface Alarm {
  id?: number
  deviceId?: string
  channelId?: string
  alarmLevel?: string
  alarmMethod?: string
  alarmType?: string
  alarmTime?: string
  alarmDescription?: string
  longitude?: number
  latitude?: number
  handled?: boolean
  handleTime?: string
  handleUser?: string
  handleResult?: string
  snapUrl?: string
  videoUrl?: string
}

export function getAlarmList(params: AlarmQueryParams) {
  return request<WvpResult<{ total: number; list: Alarm[] }>>({
    method: 'get',
    url: '/alarm/list',
    params
  })
}

export function getAlarmBefore(params: { time: string; page?: number; count?: number }) {
  return request<WvpResult<{ total: number; list: Alarm[] }>>({
    method: 'get',
    url: `/alarm/before/${params.time}`,
    params: { page: params.page, count: params.count }
  })
}

export function getAlarmDetail(id: number | string) {
  return request<WvpResult<Alarm>>({
    method: 'get',
    url: `/alarm/detail/${id}`
  })
}

export function clearAlarm(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/alarm/clear',
    params: { id }
  })
}

export function deleteAlarm(id: number | string) {
  return request<WvpResult>({
    method: 'delete',
    url: `/alarm/delete/${id}`
  })
}

export function handleAlarm(data: { id: number | string; result: string }) {
  return request<WvpResult>({
    method: 'post',
    url: '/alarm/handle',
    params: data
  })
}

export function batchAlarm(data: { ids: (number | string)[]; action: 'delete' | 'clear' | 'handle' }) {
  return request<WvpResult>({
    method: 'post',
    url: '/alarm/batch',
    data
  })
}

export function getAlarmSnapUrl(param: string) {
  return request<WvpResult<{ snapUrl: string }>>({
    method: 'get',
    url: `/alarm/snap/${encodeURIComponent(param)}`
  })
}
