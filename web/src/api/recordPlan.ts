import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

/**
 * 录像计划时段。
 *
 * 口径与 WVP-PRO 完全一致（`RecordPlanMapper.queryRecordIng`：
 * `week_day = #{week} and start <= #{index} and stop >= #{index}`）：
 *
 * - `start` / `stop`：**当天第几分钟**（0..1440，`hour*60+minute`），
 *   `1440` 表示 24:00；区间是**闭区间**，两端都含；
 * - `weekDay`：**ISO** 口径，`1`=周一 … `7`=周日（不是 0..6）。
 *
 * 早期前端用的是 `startTime/endTime/mon..sun` 那一套字段名，后端根本不认，
 * 结果"新增计划"接口返回成功但库里**一条时段都没有**，计划永远不录像。
 */
export interface RecordPlanItem {
  id?: number
  start: number
  stop: number
  weekDay: number
  planId?: number
}

export interface RecordPlan {
  id?: number
  name?: string
  /** 是否开启定时截图（WVP 保留了该字段，但服务端目前未接线） */
  snap?: boolean
  /** 关联通道数（服务端聚合返回） */
  channelCount?: number
  createTime?: string
  updateTime?: string
  planItemList?: RecordPlanItem[]
}

/** `/api/record/plan/channel/list` 返回的通道（WVP `CommonGBChannel` 子集） */
export interface RecordPlanChannel {
  id: number
  /** 通道主键；`link` 时原样回传（WVP 的前端就是这样用的） */
  gbId: number
  gbDeviceId?: string | null
  gbName?: string | null
  gbManufacturer?: string | null
  gbModel?: string | null
  gbStatus?: string | null
  dataType?: number
  recordPlanId?: number | null
}

export interface RecordPlanQueryParams {
  page: number
  count: number
  query?: string
}

export function getRecordPlanList(params: RecordPlanQueryParams) {
  return request<WvpResult<{ total: number; list: RecordPlan[] }>>({
    method: 'get',
    url: '/record/plan/query',
    params
  })
}

/** WVP 的查询参数名是 `planId`（不是 `id`） */
export function getRecordPlanOne(planId: number | string) {
  return request<WvpResult<RecordPlan | null>>({
    method: 'get',
    url: '/record/plan/get',
    params: { planId }
  })
}

export function addRecordPlan(data: {
  name: string
  snap?: boolean
  planItemList: RecordPlanItem[]
}) {
  return request<WvpResult>({
    method: 'post',
    url: '/record/plan/add',
    data
  })
}

export function updateRecordPlan(data: {
  id: number | string
  name?: string
  snap?: boolean
  planItemList?: RecordPlanItem[]
}) {
  return request<WvpResult>({
    method: 'post',
    url: '/record/plan/update',
    data
  })
}

/**
 * 删除计划。**必须用 DELETE**：后端只注册了 `delete(...)`，
 * 早期前端用 GET 调用，线上稳定返回 405，删除功能实际不可用。
 */
export function deleteRecordPlan(planId: number | string) {
  return request<WvpResult>({
    method: 'delete',
    url: '/record/plan/delete',
    params: { planId }
  })
}

export interface PlanChannelQueryParams {
  page: number
  count: number
  planId: number | string
  query?: string
  online?: string
  channelType?: string | number
  hasLink?: 'true' | 'false'
}

export function queryPlanChannels(params: PlanChannelQueryParams) {
  return request<WvpResult<{ total: number; list: RecordPlanChannel[] }>>({
    method: 'get',
    url: '/record/plan/channel/list',
    params
  })
}

/**
 * 关联 / 取消关联通道。
 *
 * - `channelIds`：通道**主键**列表（`RecordPlanChannel.gbId`）；
 * - `deviceDbIds`：设备主键列表，会关联该设备下所有通道；
 * - `allLink`：`true` 全部关联、`false` 全部取消关联；
 * - `planId` 省略（配合 `channelIds`）表示"取消这些通道的关联"。
 */
export function linkPlan(data: {
  planId?: number
  channelIds?: number[]
  deviceDbIds?: number[]
  allLink?: boolean
}) {
  return request<WvpResult>({
    method: 'post',
    url: '/record/plan/link',
    data
  })
}

// ============ 时间口径转换 ============

/** "HH:mm" → 当天第几分钟 */
export function timeStrToMinutes(v: string): number {
  const [h, m] = v.split(':')
  return Number(h) * 60 + Number(m)
}

/** 当天第几分钟 → "HH:mm"（1440 → "24:00"） */
export function minutesToTimeStr(min: number): string {
  const h = Math.floor(min / 60)
  const m = min % 60
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`
}

/** 周一..周日（ISO 1..7） */
export const WEEK_DAY_LABELS = [
  { value: 1, label: '周一' },
  { value: 2, label: '周二' },
  { value: 3, label: '周三' },
  { value: 4, label: '周四' },
  { value: 5, label: '周五' },
  { value: 6, label: '周六' },
  { value: 7, label: '周日' }
] as const

/** 把 `planItemList` 汇总成人类可读的一行，用于列表展示 */
export function summarizePlanItems(items?: RecordPlanItem[]): string {
  if (!items || items.length === 0) return '—'
  return items
    .slice()
    .sort((a, b) => a.weekDay - b.weekDay || a.start - b.start)
    .map(
      (it) =>
        `${WEEK_DAY_LABELS.find((w) => w.value === it.weekDay)?.label ?? `周?(${it.weekDay})`} ` +
        `${minutesToTimeStr(it.start)}-${minutesToTimeStr(it.stop)}`
    )
    .join('；')
}
