import { request } from '@/utils/request'
import type { ApiResult } from '@/types/api'

/**
 * API Key 管理（对接方用密钥调用后端 HTTP 接口）。
 *
 * 后端 7 条端点全部是**仅管理员**可用（`authz::require_admin`）。
 *
 * 一个重要约定：**列表里的密钥永远是掩码**（`******`）——后端只在
 * 「新增」与「重置」这两个动作的响应里返回真实密钥。因此页面必须
 * 在这两种场景把密钥完整展示出来让操作者复制，事后无处可查。
 */
export interface ApiKeyItem {
  id: number
  userId?: number | null
  app?: string | null
  /** 列表里恒为 `******`；真实值只在新增/重置的响应里出现。 */
  apiKey?: string | null
  /** 过期时间（Unix 秒）。`null` = 永不过期。 */
  expiredAt?: number | null
  remark?: string | null
  enable?: boolean | null
  createTime?: string | null
  updateTime?: string | null
}

export interface ApiKeyListResult {
  total: number
  list: ApiKeyItem[]
}

/** 新增/重置时返回的真实密钥。 */
export interface ApiKeySecretResult {
  apiKey: string
  enable?: boolean
  expiredAt?: number | null
}

export interface ApiKeyListParams {
  page?: number
  count?: number
}

export interface ApiKeyAddPayload {
  /** 归属用户；不传默认 1（管理员）。 */
  userId?: number
  /** 应用标识，便于对接方区分用途。 */
  app?: string
  /** 是否启用；不传默认 true。 */
  enable?: boolean
  /**
   * 过期时间。支持 `YYYY-MM-DD HH:mm:ss` 或 Unix 秒；不传 = 永不过期。
   */
  expiresAt?: string
  remark?: string
}

export function listApiKeys(params: ApiKeyListParams) {
  return request<ApiResult<ApiKeyListResult>>({
    method: 'get',
    url: '/userApiKey/userApiKeys',
    params
  })
}

export function addApiKey(payload: ApiKeyAddPayload) {
  return request<ApiResult<ApiKeySecretResult>>({
    method: 'post',
    url: '/userApiKey/add',
    params: payload
  })
}

export function setApiKeyRemark(id: number, remark: string) {
  return request<ApiResult>({
    method: 'post',
    url: '/userApiKey/remark',
    params: { id, remark }
  })
}

export function enableApiKey(id: number) {
  return request<ApiResult>({
    method: 'post',
    url: '/userApiKey/enable',
    params: { id }
  })
}

export function disableApiKey(id: number) {
  return request<ApiResult>({
    method: 'post',
    url: '/userApiKey/disable',
    params: { id }
  })
}

/** 重置密钥：旧的立即失效，新的只在这次响应里返回。 */
export function resetApiKey(id: number) {
  return request<ApiResult<ApiKeySecretResult>>({
    method: 'post',
    url: '/userApiKey/reset',
    params: { id }
  })
}

export function deleteApiKey(id: number) {
  return request<ApiResult>({
    method: 'delete',
    url: '/userApiKey/delete',
    params: { id }
  })
}
