import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface RecordPlanParams {
  page?: number
  count?: number
  query?: string
}

export interface RecordPlan {
  id?: number
  name?: string
  planType?: string
  startTime?: string
  endTime?: string
  enable?: boolean
  enableTime?: boolean
  mon?: boolean
  tue?: boolean
  wed?: boolean
  thu?: boolean
  fri?: boolean
  sat?: boolean
  sun?: boolean
  channelCount?: number
  createTime?: string
  updateTime?: string
}

export function getRecordPlanList(params: RecordPlanParams) {
  return request<WvpResult<{ total: number; list: RecordPlan[] }>>({
    method: 'get',
    url: '/record/plan/query',
    params
  })
}

export function getRecordPlanOne(id: number | string) {
  return request<WvpResult<RecordPlan>>({
    method: 'get',
    url: '/record/plan/get',
    params: { id }
  })
}

export function addRecordPlan(data: Partial<RecordPlan>) {
  return request<WvpResult>({
    method: 'post',
    url: '/record/plan/add',
    data
  })
}

export function updateRecordPlan(data: Partial<RecordPlan>) {
  return request<WvpResult>({
    method: 'post',
    url: '/record/plan/update',
    data
  })
}

export function deleteRecordPlan(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/record/plan/delete',
    params: { id }
  })
}

export function linkChannels(planId: number | string, channelIds: (number | string)[]) {
  return request<WvpResult>({
    method: 'post',
    url: '/record/plan/link',
    data: { planId, channelIds }
  })
}

export function unlinkChannel(planId: number | string, channelId: number | string) {
  return request<WvpResult>({
    method: 'post',
    url: '/record/plan/link',
    data: { planId, channelId }
  })
}

export function getPlanChannels(planId: number | string) {
  return request<WvpResult<{ total: number; list: { channelId: string; deviceId: string; name: string }[] }>>({
    method: 'get',
    url: '/record/plan/channel/list',
    params: { planId }
  })
}
