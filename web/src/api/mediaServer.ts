import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

/**
 * 流媒体节点。
 *
 * 此前 4 处契约与后端对不上：
 *   1. `checkMediaServer(id)` 只发 `id`，而后端 DTO 里根本没有 `id` 字段
 *      → 无论点哪个节点都在探测兜底的 `127.0.0.1:80`；
 *   2. 「检测」把响应当 `{code,msg}` 判断，而后端返回的是节点信息 payload
 *      → 无论成功与否都弹「检测失败」；
 *   3. `getMediaLoad(id)` 声明返回 `{load:number}`，实际是**数组**
 *      → 调用方只能强转类型绕过，取值恒为 undefined；
 *   4. `getMediaInfo(id)` 只发 `id`，而后端要 `app` + `stream`（+ 可选节点）
 *      → 恒 400「缺少 app 或 stream 参数」。
 */

export interface MediaServer {
  id?: string
  ip: string
  httpPort: number
  hookIp?: string
  sdpIp?: string
  streamIp?: string
  httpSslPort?: number
  rtspPort?: number
  rtmpPort?: number
  rtpProxyPort?: number
  secret: string
  type?: string
  autoConfig?: boolean
  rtpEnable?: boolean
  rtpPortRange?: string
  sendRtpPortRange?: string
  recordAssistPort?: number
  defaultServer?: boolean
  /** 是否参与选路（false = 暂时下线，不接收新流） */
  enabled?: boolean
  /** 健康检查得出的在线状态（与 enabled 是两件事） */
  status?: boolean
  lastKeepaliveTime?: string
  hookAliveInterval?: number
  recordPath?: string
  recordDay?: number
  transcodeSuffix?: string
  serverId?: string
  createTime?: string
  updateTime?: string
}

/** `check` 的返回：节点探测结果 */
export interface MediaServerProbe {
  reachable?: boolean
  probeError?: string
  ip?: string
  httpPort?: number
  secret?: string
  type?: string
  streamIp?: string
  sdpIp?: string
  hookIp?: string
  rtpPortRange?: string
  sendRtpPortRange?: string
  [key: string]: unknown
}

/** 单节点的实时负载 */
export interface MediaServerLoad {
  id: string
  push: number
  proxy: number
  gbReceive: number
  gbSend: number
}

export interface MediaInfo {
  app: string
  stream: string
  schema?: string
  vhost?: string
  readerCount?: number
  totalReaderCount?: number
  originType?: number
  originUrl?: string
  createStamp?: number
  aliveSecond?: number
  bytesSpeed?: number
}

export function getMediaServerList() {
  return request<WvpResult<MediaServer[]>>({
    method: 'get',
    url: '/server/media_server/list'
  })
}

export function getMediaServerOnlineList() {
  return request<WvpResult<MediaServer[]>>({
    method: 'get',
    url: '/server/media_server/online/list'
  })
}

export function getMediaServerOne(id: string) {
  return request<WvpResult<MediaServer>>({
    method: 'get',
    url: `/server/media_server/one/${id}`
  })
}

export function saveMediaServer(data: Partial<MediaServer>) {
  return request<WvpResult>({
    method: 'post',
    url: '/server/media_server/save',
    data
  })
}

export function deleteMediaServer(id: string) {
  return request<WvpResult>({
    method: 'delete',
    url: '/server/media_server/delete',
    params: { id }
  })
}

/**
 * 检测节点连通性。传 `id` 时后端会用该节点**自己**的 ip/端口/secret 去探测；
 * 也支持显式传 ip/port/secret/type（新增节点时"先探测再保存"）。
 * 探测失败后端会返回业务错误，axios 拦截器会把 msg 抛出来。
 */
export function checkMediaServer(params: {
  id?: string
  ip?: string
  port?: number
  secret?: string
  type?: string
}) {
  return request<WvpResult<MediaServerProbe>>({
    method: 'get',
    url: '/server/media_server/check',
    params
  })
}

/**
 * 节点负载。不传 `id` 返回全部节点（WVP 语义），传 `id` 只返回该节点。
 * 返回的是**数组**，调用方按 `id` 取自己那一项。
 */
export function getMediaLoad(id?: string) {
  return request<WvpResult<MediaServerLoad[]>>({
    method: 'get',
    url: '/server/media_server/load',
    params: id ? { id } : undefined
  })
}

/** 单条流信息（后端返回的就是一条 MediaInfo，不是列表） */
export function getMediaInfo(app: string, stream: string, mediaServerId?: string) {
  return request<WvpResult<MediaInfo>>({
    method: 'get',
    url: '/server/media_server/media_info',
    params: { app, stream, mediaServerId }
  })
}
