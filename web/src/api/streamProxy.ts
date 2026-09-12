import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

/**
 * 拉流代理。
 *
 * 字段名与后端 `gb_stream_proxy` / WVP 的 `StreamProxy` bean **严格一致**
 * （camelCase）。早期这里用的是 `url` / `enabled` / `status` / `destUrl` 四个
 * 后端根本不存在的键，后果是：列表"源 URL"整列空白、启用开关与状态永远显示
 * 关闭、"目标 URL"填了也没人收。
 */
export interface StreamProxyQueryParams {
  page?: number
  count?: number
  /** 关键字：app / stream / name / srcUrl / type 模糊匹配 */
  query?: string
  /** 是否正在拉流；不传或传空串 = 全部 */
  pulling?: boolean | string
  mediaServerId?: string
}

export interface StreamProxy {
  id?: number
  name: string
  /** default = ZLM 原生拉流；ffmpeg = 交给 FFmpeg 转发 */
  type?: 'default' | 'ffmpeg' | string
  app?: string
  stream?: string
  /** 源地址（RTSP/RTMP/HLS） */
  srcUrl?: string
  /** 用户选择的节点；auto 或空表示自动 */
  relatesMediaServerId?: string
  /** 实际承载该代理的节点 */
  mediaServerId?: string
  /** 拉流成功超时（秒） */
  timeout?: number
  /** type=ffmpeg 时的命令模板键，取自节点 ffmpeg.cmd* */
  ffmpegCmdKey?: string
  /** RTSP 拉流方式："0" TCP / "1" UDP / "2" 组播 */
  rtspType?: string
  /** 是否启用该代理 */
  enable?: boolean
  enableAudio?: boolean
  enableMp4?: boolean
  enableDisableNoneReader?: boolean
  /** 是否正在拉流 */
  pulling?: boolean
  /** Phase 4.5 统一流状态：ready/active/... */
  streamStatus?: string
  createTime?: string
  updateTime?: string
}

export interface StreamProxyListPage {
  total: number
  list: StreamProxy[]
  pageNum?: number
  pageSize?: number
  pages?: number
}

export function getStreamProxyList(params: StreamProxyQueryParams) {
  return request<WvpResult<StreamProxyListPage>>({
    method: 'get',
    url: '/proxy/list',
    params
  })
}

export function getStreamProxyOne(id: number | string) {
  return request<WvpResult<StreamProxy>>({
    method: 'get',
    url: '/proxy/one',
    params: { id }
  })
}

/** WVP 的 `/proxy/one` 签名是按 app+stream 查 */
export function getStreamProxyByAppStream(app: string, stream: string) {
  return request<WvpResult<StreamProxy>>({
    method: 'get',
    url: '/proxy/one',
    params: { app, stream }
  })
}

/** 节点上可用的 ffmpeg.cmd* 模板（真实读 ZLM getServerConfig） */
export function getFfmpegCmdList(mediaServerId: string) {
  return request<WvpResult<Record<string, string>>>({
    method: 'get',
    url: '/proxy/ffmpeg_cmd/list',
    params: { mediaServerId }
  })
}

export function addStreamProxy(data: Partial<StreamProxy>) {
  return request<WvpResult<StreamProxy>>({
    method: 'post',
    url: '/proxy/add',
    data
  })
}

export function updateStreamProxy(data: Partial<StreamProxy>) {
  return request<WvpResult<StreamProxy>>({
    method: 'post',
    url: '/proxy/update',
    data
  })
}

/** 保存：同 app+stream 已存在则更新 */
export function saveStreamProxy(data: Partial<StreamProxy>) {
  return request<WvpResult<StreamProxy>>({
    method: 'post',
    url: '/proxy/save',
    data
  })
}

export function startStreamProxy(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/proxy/start',
    params: { id }
  })
}

export function stopStreamProxy(id: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: '/proxy/stop',
    params: { id }
  })
}

export function deleteStreamProxy(id: number | string) {
  return request<WvpResult>({
    method: 'delete',
    url: '/proxy/delete',
    params: { id }
  })
}

/** WVP 的 `/proxy/del`：按 app+stream 删除 */
export function deleteStreamProxyByAppStream(app: string, stream: string) {
  return request<WvpResult>({
    method: 'delete',
    url: '/proxy/del',
    params: { app, stream }
  })
}
