import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export function startPlay(deviceId: string, channelId: string) {
  return request<WvpResult<{ streamId: string; playUrl: string }>>({
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

export function getWebrtcPlay(params: { deviceId: string; channelId: string }) {
  return request<WvpResult<{ url: string }>>({
    method: 'get',
    url: '/play/webrtc',
    params
  })
}

export function queryStreams(params: { page?: number; count?: number; query?: string }) {
  return request<WvpResult<{ total: number; list: { mediaServerId: string; app: string; stream: string; readerCount?: number }[] }>>({
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
  cmd: string
  speed?: number
}) {
  return request<WvpResult>({
    method: 'get',
    url: `/front-end/ptz/${params.deviceId}/${params.channelId}`,
    params: { cmd: params.cmd, speed: params.speed ?? 50 }
  })
}

export interface StreamChannel {
  deviceId: string
  channelId: string
  name: string
  online: boolean
  streamUrl?: string
}
