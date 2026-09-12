import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

/**
 * 级联平台。
 *
 * 字段名与后端 `gb_platform` / WVP 的 `Platform.java` **严格一致**（`serverGBId`
 * 里是大写 `B`）。此前这里写的是 `serverGbId`（小写 b），于是：
 * 新增时后端收不到国标ID（写空串，平台实际不可用）、列表"国标ID"列空白、
 * 「注销」按钮因为 `if (!row.serverGbId) return` 静默失效。
 *
 * 「注册间隔 / 心跳间隔 / 心跳次数」三个输入框是**凭空发明**的字段（WVP 与
 * 数据库都没有），已换成真实存在的 `expires`（注册周期）与 `keepTimeout`（心跳周期）。
 */
export interface PlatformQueryParams {
  page?: number
  count?: number
  query?: string
}

export interface Platform {
  id?: number
  /** SIP 服务国标编码（20 位） */
  serverGBId: string
  /** SIP 服务国标域 */
  serverGBDomain?: string
  name?: string
  serverIp?: string
  serverPort?: number
  deviceGBId?: string
  deviceIp?: string
  devicePort?: string
  username?: string
  password?: string
  /** 注册周期（秒） */
  expires?: number
  /** 心跳周期（秒），WVP 字段名 keepTimeout */
  keepTimeout?: number
  transport?: string
  characterSet?: string
  civilCode?: string
  manufacturer?: string
  model?: string
  address?: string
  ptz?: boolean
  rtcp?: boolean
  enable?: boolean
  status?: boolean
  catalogGroup?: number
  registerWay?: number
  secrecy?: number
  asMessageChannel?: boolean
  autoPushChannel?: boolean
  catalogWithPlatform?: number
  catalogWithGroup?: number
  catalogWithRegion?: number
  sendStreamIp?: string
  serverId?: string
  channelCount?: number
  createTime?: string
  updateTime?: string
}

export interface PlatformServerConfig {
  id?: number | null
  name?: string
  /** 兼容旧声明：与 serverIp / serverPort 同值 */
  ip?: string
  port?: number
  realm?: string
  serverGBId?: string
  serverGBDomain?: string
  serverHost?: string
  serverIp?: string
  serverPort?: number
  deviceIp?: string
  devicePort?: string
  username?: string
  password?: string
  transport?: string
  sendStreamIp?: string
}

export function getPlatformList(params: PlatformQueryParams) {
  return request<WvpResult<{ total: number; list: Platform[] }>>({
    method: 'get',
    url: '/platform/query',
    params
  })
}

export function getPlatformOne(id: number | string) {
  return request<WvpResult<Platform>>({
    method: 'get',
    url: `/platform/info/${id}`
  })
}

export function addPlatform(data: Partial<Platform>) {
  return request<WvpResult>({
    method: 'post',
    url: '/platform/add',
    data
  })
}

export function updatePlatform(data: Partial<Platform>) {
  return request<WvpResult>({
    method: 'post',
    url: '/platform/update',
    data
  })
}

export function deletePlatform(id: number | string) {
  return request<WvpResult>({
    method: 'delete',
    url: '/platform/delete',
    params: { id }
  })
}

/** 向上级平台发送 Expires:0 的注销 REGISTER，并把该平台置为停用 */
export function platformExit(serverGBId: string) {
  return request<WvpResult>({
    method: 'get',
    url: `/platform/exit/${serverGBId}`
  })
}

export function getPlatformServerConfig() {
  return request<WvpResult<PlatformServerConfig>>({
    method: 'get',
    url: '/platform/server_config'
  })
}

export interface PlatformCatalogBody {
  name: string
  parentId?: number | string
  civilCode?: string
  businessGroup?: string
}

export function addPlatformCatalog(data: PlatformCatalogBody & { platformId: number | string }) {
  return request<WvpResult>({
    method: 'post',
    url: '/platform/catalog/add',
    data
  })
}

export function editPlatformCatalog(data: PlatformCatalogBody & { id: number | string }) {
  return request<WvpResult>({
    method: 'post',
    url: '/platform/catalog/edit',
    data
  })
}
