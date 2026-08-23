/**
 * 后端 WVPResult 标准响应：{ code, msg, data }
 */
export interface WvpResult<T = unknown> {
  code: number
  msg: string
  data: T
}

export interface PageQuery {
  page?: number
  count?: number
}

export interface PageResult<T> {
  total: number
  list: T[]
}

/** 字典类型（GB/T 28181 数据源标识） */
export type DataType = 1 | 2 | 3 | 200
