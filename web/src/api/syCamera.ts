import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface CameraItem {
  id: number
  device_id: string
  channel_id: string
  parent_device_id?: string
  name?: string
  manufacturer?: string
  model?: string
  owner?: string
  civil_code?: string
  address?: string
  status?: string
  online?: boolean
  has_audio?: boolean
  longitude?: number
  latitude?: number
  sub_count?: number
}

export interface CameraListResponse {
  /** 匹配的**设备**总数（与 page/count 同为设备维度；带 civilCode 时是匹配行数） */
  total: number
  /** 本次实际返回的行数（设备展开成通道后） */
  listTotal?: number
  count: number
  page: number
  list: CameraItem[]
}

/**
 * 取所有摄像机（含通道），构建设备 / 通道树用
 * 与原 WVP Java 版 /api/sy/camera/list-with-child 兼容
 */
export function cameraListWithChild(params: {
  page?: number
  count?: number
  query?: string
  online?: boolean
  civilCode?: string
} = {}) {
  return request<WvpResult<CameraListResponse>>({
    method: 'get',
    url: '/sy/camera/list-with-child',
    params,
  })
}

/**
 * 取摄像机通道列表（WVP 同名端点返回的也是通道）。
 *
 * 注意：后端现在返回的是**通道级**行（设备没有通道时用设备自身那一行，
 * `is_device = true`）。此前它返回的是纯设备行，照 live 页的
 * `!c.is_device` 口径会被整批滤掉 → 通道树为空。
 */
export function cameraList(params: {
  page?: number
  count?: number
  query?: string
  online?: boolean
} = {}) {
  return request<WvpResult<CameraListResponse>>({
    method: 'get',
    url: '/sy/camera/list',
    params,
  })
}