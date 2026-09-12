import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

/**
 * 行政区划（gb_common_region）与业务分组（gb_common_group）。
 *
 * 字段名与后端 / WVP 的 `Region.java` / `Group.java` 一致（camelCase）。
 * 此前两处契约错误：
 *   1. `deleteRegion` / `deleteGroup` 用 GET，后端只注册 DELETE → 405，删除永远失败；
 *   2. `getRegionTreeQuery` / `getGroupTreeQuery` 声明返回 `Region[]`，
 *      而后端返回的是分页对象 `{total, list}`（与 WVP 的 `PageInfo` 一致）
 *      → 任何 `res.data.map(...)` 都会在运行时炸。
 */

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
  children?: Region[]
}

export interface Group {
  id?: number
  deviceId?: string
  name: string
  parentId?: number
  parentName?: string
  businessGroup?: string
  civilCode?: string
  path?: string
  treePath?: string
  createTime?: string
  updateTime?: string
  children?: Group[]
}

/** 树查询响应：PageInfo 形状 */
export interface TreeNodePage<T> {
  total: number
  list: T[]
  pageNum?: number
  pageSize?: number
  pages?: number
}

export interface TreeNodeQueryParams {
  page?: number
  count?: number
  /** 关键字（名称 / 国标编码），对应 WVP 的 `query` */
  query?: string
  /** 取该父节点下的子节点；`-1` 表示顶级节点 */
  parentId?: number
}

// ---------- 行政区划 ----------

export function getRegionTreeList() {
  return request<WvpResult<Region[]>>({
    method: 'get',
    url: '/region/tree/list'
  })
}

export function getRegionTreeQuery(params: TreeNodeQueryParams = {}) {
  return request<WvpResult<TreeNodePage<Region>>>({
    method: 'get',
    url: '/region/tree/query',
    params
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
    method: 'delete',
    url: '/region/delete',
    params: { id }
  })
}

/** 拉取公安部行政区划数据 */
export function syncRegion() {
  return request<WvpResult<{ count: number }>>({
    method: 'get',
    url: '/region/sync'
  })
}

/** 按行政区划代码自动建区域（后端已有 /region/addByCivilCode） */
export function addRegionByCivilCode(civilCode: string) {
  return request<WvpResult>({
    method: 'get',
    url: '/region/addByCivilCode',
    params: { civilCode }
  })
}

// ---------- 业务分组 ----------

export function getGroupTreeList() {
  return request<WvpResult<Group[]>>({
    method: 'get',
    url: '/group/tree/list'
  })
}

export function getGroupTreeQuery(params: TreeNodeQueryParams = {}) {
  return request<WvpResult<TreeNodePage<Group>>>({
    method: 'get',
    url: '/group/tree/query',
    params
  })
}

export function getGroupOne(id: number) {
  return request<WvpResult<Group>>({
    method: 'get',
    url: '/group/one',
    params: { id }
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
    method: 'delete',
    url: '/group/delete',
    params: { id }
  })
}
