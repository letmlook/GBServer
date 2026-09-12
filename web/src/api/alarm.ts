import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface AlarmQueryParams {
  page?: number
  count?: number
  /** 关键字：设备号/通道号/描述（后端扩展，WVP 无此参数） */
  query?: string
  /** WVP 的时间参数名是 beginTime/endTime（后端同时接受 startTime） */
  beginTime?: string
  endTime?: string
  alarmType?: string
  deviceId?: string
  channelId?: string
  handled?: boolean
}

export interface Alarm {
  id?: number
  deviceId?: string
  channelId?: string
  /**
   * 报警级别（GB/T 28181 报警优先级：1 一级/紧急 … 4 四级/提示）。
   * 后端字段名是 `alarmPriority`；早期前端读 `alarmLevel`，后端从不返回该键，
   * 所以「级别」列恒为空、仪表盘的等级标签恒为兜底「信息」。
   */
  alarmPriority?: string
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

/**
 * 删除某时间之前的告警。
 *
 * 后端 `/api/alarm/before/:time` 只注册了 **DELETE**，且返回 `{deleted}`；
 * 早期这里写成 GET 并声明返回 `{total,list}` —— 调用必然 405，
 * 即使改成 DELETE 也读不到列表（它本来就是删除接口）。
 */
export function deleteAlarmsBefore(time: string) {
  return request<WvpResult<{ deleted: number }>>({
    method: 'delete',
    url: `/alarm/before/${encodeURIComponent(time)}`
  })
}

export function getAlarmDetail(id: number | string) {
  return request<WvpResult<Alarm>>({
    method: 'get',
    url: `/alarm/detail/${id}`
  })
}

/**
 * 按筛选条件清空告警（WVP `DELETE /api/alarm/clear?alarmType&beginTime&endTime`）。
 *
 * 注意：这个接口**不接受单个 id**（WVP 也不接受）；清空某一类/某段时间的
 * 告警用它，删除「选中的若干条」用 `deleteAlarms(ids)`。
 * 此前前端用 GET 调它 → 405；而后端实现又是无条件的全表删除。
 */
export function clearAlarms(params?: {
  alarmType?: string
  beginTime?: string
  endTime?: string
  deviceId?: string
  channelId?: string
  query?: string
}) {
  return request<WvpResult<{ cleared: number }>>({
    method: 'delete',
    url: '/alarm/clear',
    params: params ?? {}
  })
}

export function deleteAlarm(id: number | string) {
  return request<WvpResult>({
    method: 'delete',
    url: `/alarm/delete/${id}`
  })
}

/**
 * 处理告警。后端要求 **JSON body**（`Json<AlarmHandleBody>`）；
 * 早期把参数放在 query 上、body 为空 → axum 直接 415，处理结论永远提交不了。
 */
export function handleAlarm(data: {
  id: number | string
  result?: string
  handleUser?: string
  handled?: boolean
}) {
  return request<WvpResult>({
    method: 'post',
    url: '/alarm/handle',
    data: { handled: true, ...data }
  })
}

/**
 * 批量删除告警（WVP 契约：`DELETE /api/alarm/delete`，body 是**裸数组**）。
 *
 * 早期实现是 `POST /alarm/batch` + `{ids, action}`：后端只注册了 DELETE，
 * 必然 405；而且后端根本不认 `action`，「批量清除」会变成永久删除。
 * 这里只保留"删除"语义，方法/端点/体型全部对齐 WVP。
 */
export function deleteAlarms(ids: (number | string)[]) {
  return request<WvpResult<{ deleted: number }>>({
    method: 'delete',
    url: '/alarm/delete',
    data: ids
  })
}

/** 级别码 → 中文名（GB/T 28181 报警优先级） */
export const ALARM_PRIORITY_LABELS: Record<string, string> = {
  '1': '一级（紧急）',
  '2': '二级（重要）',
  '3': '三级（一般）',
  '4': '四级（提示）'
}

export function alarmPriorityLabel(priority?: string | number | null): string {
  if (priority === undefined || priority === null || priority === '') return '—'
  return ALARM_PRIORITY_LABELS[String(priority)] ?? `优先级 ${priority}`
}

export function getAlarmSnapUrl(param: string) {
  return request<WvpResult<{ snapUrl: string }>>({
    method: 'get',
    url: `/alarm/snap/${encodeURIComponent(param)}`
  })
}
