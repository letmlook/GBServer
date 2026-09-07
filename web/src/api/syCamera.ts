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
  total: number
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
 * 取所有摄像机（含通道）的另一别名（不带分页）
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