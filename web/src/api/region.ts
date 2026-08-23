import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface Region {
  id?: number
  deviceId?: string
  name: string
  parentId?: number
  parentName?: string
  path?: string
  treePath?: string
  civilCode?: string
  createTime?: string
  updateTime?: string
}

export function getRegionTreeList() {
  return request<WvpResult<Region[]>>({
    method: 'get',
    url: '/region/tree/list'
  })
}

export function getRegionTreeQuery(parentId?: number) {
  return request<WvpResult<Region[]>>({
    method: 'get',
    url: '/region/tree/query',
    params: { parentId }
  })
}

export function getRegionPath(id: number) {
  return request<WvpResult<Region[]>>({
    method: 'get',
    url: '/region/path',
    params: { id }
  })
}

export function getRegionOne(id: number) {
  return request<WvpResult<Region>>({
    method: 'get',
    url: '/region/one',
    params: { id }
  })
}

export function addRegion(data: Partial<Region>) {
  return request<WvpResult>({
    method: 'post',
    url: '/region/add',
    data
  })
}

export function updateRegion(data: Partial<Region>) {
  return request<WvpResult>({
    method: 'post',
    url: '/region/update',
    data
  })
}

export function deleteRegion(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/region/delete',
    params: { id }
  })
}

export function syncRegion() {
  return request<WvpResult<{ count: number }>>({
    method: 'get',
    url: '/region/sync'
  })
}

export interface Group {
  id?: number
  deviceId?: string
  name: string
  parentId?: number
  parentName?: string
  path?: string
  treePath?: string
  createTime?: string
  updateTime?: string
}

export function getGroupTreeList() {
  return request<WvpResult<Group[]>>({
    method: 'get',
    url: '/group/tree/list'
  })
}

export function getGroupTreeQuery(parentId?: number) {
  return request<WvpResult<Group[]>>({
    method: 'get',
    url: '/group/tree/query',
    params: { parentId }
  })
}

export function addGroup(data: Partial<Group>) {
  return request<WvpResult>({
    method: 'post',
    url: '/group/add',
    data
  })
}

export function updateGroup(data: Partial<Group>) {
  return request<WvpResult>({
    method: 'post',
    url: '/group/update',
    data
  })
}

export function deleteGroup(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/group/delete',
    params: { id }
  })
}
