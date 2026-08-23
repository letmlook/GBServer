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
  return request<WvpResult<string[]>>({
    method: 'get',
    url: '/common/channel/industry/list'
  })
}

export function getTypeList() {
  return request<WvpResult<string[]>>({
    method: 'get',
    url: '/common/channel/type/list'
  })
}

export function getNetworkIdentificationList() {
  return request<WvpResult<string[]>>({
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

export function changeAudio(channelId: string, audio: boolean) {
  return request<WvpResult>({
    method: 'post',
    url: '/common/channel/play',
    params: { channelId, audio }
  })
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
