import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface ChannelListParams {
  page?: number
  count?: number
  query?: string
  online?: boolean
  channelType?: number
  catalogUnderDevice?: boolean
  deviceId?: string
}

export function getChannelList(params: ChannelListParams) {
  return request<WvpResult<{ total: number; list: Channel[] }>>({
    method: 'get',
    url: '/common/channel/list',
    params
  })
}

export function getChannelOne(id: string | number) {
  return request<WvpResult<Channel>>({
    method: 'get',
    url: '/common/channel/one',
    params: { id }
  })
}

export function getIndustryList() {
  return request<WvpResult<ChannelCodeType[]>>({
    method: 'get',
    url: '/common/channel/industry/list'
  })
}

export function getTypeList() {
  return request<WvpResult<ChannelCodeType[]>>({
    method: 'get',
    url: '/common/channel/type/list'
  })
}

export function getNetworkIdentificationList() {
  return request<WvpResult<ChannelCodeType[]>>({
    method: 'get',
    url: '/common/channel/network/identification/list'
  })
}

export function addChannel(data: Partial<Channel>) {
  return request<WvpResult>({
    method: 'post',
    url: '/common/channel/add',
    data
  })
}

/**
 * 单通道删除（DELETE /api/common/channel/delete?id=<channel.id>）
 * 注意：依赖列表查询返回的 `id` 字段（数据库主键），不是 channelId（国标 ID）
 */
export function deleteChannel(id: number | string) {
  return request<WvpResult>({
    method: 'delete',
    url: '/common/channel/delete',
    params: { id }
  })
}

export function updateChannel(data: Partial<Channel>) {
  return request<WvpResult>({
    method: 'post',
    url: '/common/channel/update',
    data
  })
}

export function resetChannel(data: Partial<Channel>) {
  return request<WvpResult>({
    method: 'post',
    url: '/common/channel/reset',
    data
  })
}

/**
 * 播放通道（WVP `/api/common/channel/play` 是 **GET**，参数为通道**主键**）。
 * 此前写成 POST + 字符串国标 ID：方法不匹配直接 405。
 */
export function playChannel(channelId: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/common/channel/play',
    params: { channelId }
  })
}

export function stopChannelPlay(channelId: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/common/channel/play/stop',
    params: { channelId }
  })
}

/** 行业 / 类型 / 网络标识 都是 `{name, code}`（WVP `IndustryCodeType` 等） */
export interface ChannelCodeType {
  name: string
  code: string
  notes?: string
}

export function updateStreamIdentification(params: { deviceDbId: number | string; id: string | number; streamIdentification: string }) {
  return request<WvpResult>({
    method: 'post',
    url: '/device/query/channel/stream/identification/update/',
    params
  })
}

export interface Channel {
  id?: number
  channelId: string
  deviceId?: string
  name?: string
  manufacturer?: string
  model?: string
  owner?: string
  civilCode?: string
  address?: string
  status?: string
  parental?: string
  parentId?: string
  longitude?: number
  latitude?: number
  streamIdentification?: string
  channelType?: number
  hasAudio?: boolean
  audio?: boolean
  subCount?: number
  registerStatus?: string
  createTime?: string
  updateTime?: string
}
