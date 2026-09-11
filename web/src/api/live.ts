import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

/** /api/play/start 的实际返回（后端 play.rs 一次给出全部可用地址）。 */
export interface PlayStartResult {
  app: string
  stream: string
  streamId?: string
  playUrl: string
  flvUrl?: string
  wsUrl?: string
  ws_flv?: string
  hls?: string
  webrtc?: string
  deviceId?: string
  channelId?: string
  hasAudio?: boolean
  ssrc?: string
  transport?: string
}

/**
 * 拉起实时流：后端会发 SIP INVITE、开 ZLM RTP server，并返回各协议播放地址。
 *
 * 注意：**必须先调用本接口**再播放 —— `/api/media/getPlayUrl` 只是拼地址，
 * 它不会拉起流（流不存在时后端会明确报错）。
 */
export function startPlay(deviceId: string, channelId: string) {
  return request<WvpResult<PlayStartResult>>({
    method: 'get',
    url: `/play/start/${deviceId}/${channelId}`
  })
}

export function stopPlay(deviceId: string, channelId: string) {
  return request<WvpResult>({
    method: 'get',
    url: `/play/stop/${deviceId}/${channelId}`
  })
}

export function playSnap(deviceId: string, channelId: string) {
  return request<WvpResult<{ snapUrl: string }>>({
    method: 'get',
    url: `/play/snap/${deviceId}/${channelId}`
  })
}

export function getSsrc(deviceId: string, channelId: string) {
  return request<WvpResult<{ ssrc: string }>>({
    method: 'get',
    url: `/play/ssrc/${deviceId}/${channelId}`
  })
}

export function startBroadcast(deviceId: string, channelId: string) {
  return request<WvpResult>({
    method: 'get',
    url: `/play/broadcast/${deviceId}/${channelId}`
  })
}

export function stopBroadcast(deviceId: string, channelId: string) {
  return request<WvpResult>({
    method: 'get',
    url: `/play/broadcast/stop/${deviceId}/${channelId}`
  })
}

export function getPlayUrl(params: { deviceId: string; channelId?: string; protocol?: 'rtsp' | 'rtmp' | 'hls' | 'webrtc' }) {
  return request<WvpResult<{ url: string; streamId: string }>>({
    method: 'get',
    url: '/media/getPlayUrl',
    params
  })
}

/**
 * WebRTC 拉流：后端 `/api/play/webrtc` 只注册了 **POST**，且要求 JSON body
 * 里带 SDP offer（`WebRtcOfferRequest { deviceId, channelId, sdp, type }`），
 * 返回的是 **SDP answer**（`{sdp, type, app, stream}`），不是播放地址。
 *
 * 此前这里写成 GET + query 并声明返回 `{url}`：方法不匹配会 405，
 * 即使改成 GET 参数也进不了 body。当前无页面调用方（直播页用
 * `/api/play/start` 返回的 flv/hls/ws 地址）。
 */
export function postWebrtcPlay(params: {
  deviceId: string
  channelId: string
  sdp: string
  type?: string
}) {
  return request<WvpResult<{ sdp: string; type: string; app: string; stream: string }>>({
    method: 'post',
    url: '/play/webrtc',
    data: { ...params, type: params.type ?? 'offer' }
  })
}

export interface MediaStreamRow {
  /** 国标设备编号（由 `stream` 名 `设备ID_通道ID` 解析；解析不出为空串） */
  deviceId: string
  /** 国标通道编号 */
  channelId: string
  mediaServerId?: string
  schema?: string
  app: string
  stream: string
  vhost?: string
  readerCount?: number
  totalReaderCount?: number
  originType?: number
  aliveSecond?: number
  bytesSpeed?: number
}

export function queryStreams(params: { page?: number; count?: number; query?: string }) {
  return request<WvpResult<{ total: number; list: MediaStreamRow[] }>>({
    method: 'get',
    url: '/device/query/streams',
    params
  })
}

/**
 * 云台控制命令：LEFT/RIGHT/UP/DOWN/STOP/ZOOM_IN/ZOOM_OUT/FOCUS_NEAR/FOCUS_FAR/IRIS_OPEN/IRIS_CLOSE
 * 通过后端 SIP MESSAGE DeviceControl 下发到 GB28181 通道
 */
export function sendPtz(params: {
  deviceId: string
  channelId: string
  /** UP/DOWN/LEFT/RIGHT/STOP/ZOOM_IN/ZOOM_OUT */
  cmd: string
  /** 单一速度（0-255），三个方向共用；需要分别控制时用下面的可选参数 */
  speed?: number
  horizonSpeed?: number
  verticalSpeed?: number
  zoomSpeed?: number
}) {
  const speed = params.speed ?? 50
  return request<WvpResult>({
    method: 'get',
    url: `/front-end/ptz/${params.deviceId}/${params.channelId}`,
    // 参数名必须与 WVP/后端一致：command + 三个 *Speed。
    // 早期发的是 `cmd`/`speed`，后端一个都绑不上 —— 命令变成"无动作"，
    // 接口却返回成功，云台按钮点了没反应。
    params: {
      command: params.cmd,
      horizonSpeed: params.horizonSpeed ?? speed,
      verticalSpeed: params.verticalSpeed ?? speed,
      zoomSpeed: params.zoomSpeed ?? speed
    }
  })
}

export interface StreamChannel {
  deviceId: string
  channelId: string
  name: string
  online: boolean
  streamUrl?: string
}
