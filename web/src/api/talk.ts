import { request } from '@/utils/request'
import { getToken } from '@/utils/auth'
import type { WvpResult } from '@/types/api'

export interface TalkSession {
  callId: string
  deviceId: string
  channelId: string
  status: string
  localPort?: number
  deviceIp?: string
  devicePort?: number
  startTime?: string
}

/** 开始语音对讲（发送 SIP INVITE，等设备 200 OK 后会话变为 active）。 */
export function startTalk(deviceId: string, channelId: string) {
  return request<WvpResult<{ callId: string; status: string }>>({
    method: 'get',
    url: `/talk/start/${encodeURIComponent(deviceId)}/${encodeURIComponent(channelId)}`
  })
}

/** 停止语音对讲（发送 SIP BYE）。 */
export function stopTalk(deviceId: string, channelId: string) {
  return request<WvpResult>({
    method: 'get',
    url: `/talk/stop/${encodeURIComponent(deviceId)}/${encodeURIComponent(channelId)}`
  })
}

/** 当前所有活跃对讲会话（含本地收流端口与设备音频地址）。 */
export function listTalk() {
  return request<WvpResult<{ total: number; list: TalkSession[] }>>({
    method: 'get',
    url: '/talk/list'
  })
}

/**
 * 上行音频 WebSocket 地址。
 *
 * 二进制帧 = **8kHz 单声道 i16 小端 PCM**；服务端编码为 G.711A 后按 RTP
 * 发给设备（见后端 `sip::gb28181::talk_audio`）。
 *
 * 走 `VITE_APP_BASE_API` 前缀：开发模式下是 `/dev-api`（由 vite 代理，
 * 已在 `vite.config.ts` 里开启 `ws: true`），生产是 `/api`（同源）。
 * 鉴权用 `?token=`：浏览器无法为 WebSocket 设置自定义请求头。
 */
export function talkAudioWsUrl(deviceId: string, channelId: string): string {
  const base = (import.meta.env.VITE_APP_BASE_API || '/api').replace(/\/$/, '')
  const proto = window.location.protocol === 'https:' ? 'wss' : 'ws'
  const token = getToken() ?? ''
  return (
    `${proto}://${window.location.host}${base}/talk/audio/` +
    `${encodeURIComponent(deviceId)}/${encodeURIComponent(channelId)}` +
    `?token=${encodeURIComponent(token)}`
  )
}
