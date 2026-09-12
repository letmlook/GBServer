import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

/**
 * 回放拉流。
 *
 * 后端会同时给出 `playUrl`(rtsp) / `flvUrl` / `hls`：**浏览器不能直接播放 RTSP**，
 * 页面必须优先用 `hls`（hls.js）或 `flvUrl`（flv.js）——与实时预览页同一约定。
 * 此前这里只声明 `playUrl`，页面把它塞进 `<video src>`，画面永远出不来。
 */
export interface PlaybackStream {
  streamId: string
  deviceId?: string
  channelId?: string
  app?: string
  stream?: string
  /** rtsp://…（原生 video 播不了，仅作兼容保留） */
  playUrl?: string
  /** http://…/{app}/{stream}.flv */
  flvUrl?: string
  /** http://…/{app}/{stream}/hls.m3u8 */
  hls?: string
  startTime?: string
  endTime?: string
  currentTime?: string
  speed?: number
  source?: string
}

export function startPlayback(deviceId: string, channelId: string, params: { startTime?: string; endTime?: string }) {
  return request<WvpResult<PlaybackStream>>({
    method: 'get',
    url: `/playback/start/${deviceId}/${channelId}`,
    params
  })
}

export function stopPlayback(deviceId: string, channelId: string, streamId: string) {
  return request<WvpResult>({
    method: 'get',
    url: `/playback/stop/${deviceId}/${channelId}/${streamId}`
  })
}

export function pausePlayback(streamId: string) {
  return request<WvpResult>({
    method: 'get',
    url: `/playback/pause/${streamId}`
  })
}

export function resumePlayback(streamId: string) {
  return request<WvpResult>({
    method: 'get',
    url: `/playback/resume/${streamId}`
  })
}

export function seekPlayback(streamId: string, seekTime: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: `/playback/seek/${streamId}/${seekTime}`
  })
}

export function speedPlayback(streamId: string, speed: number | string) {
  return request<WvpResult>({
    method: 'get',
    url: `/playback/speed/${streamId}/${speed}`
  })
}

export function queryGbRecord(params: { deviceId: string; channelId: string; startTime?: string; endTime?: string }) {
  return request<WvpResult<{ total: number; list: RecordItem[] }>>({
    method: 'get',
    url: `/gb_record/query/${params.deviceId}/${params.channelId}`,
    params: { startTime: params.startTime, endTime: params.endTime }
  })
}

export interface RecordItem {
  deviceId: string
  channelId: string
  /** 展示名（ZLM MP4 兜底分支也返回它，此前只给 fileName → 名称列整列空白） */
  name: string
  /** ZLM 分支的原始文件名 */
  fileName?: string
  filePath?: string
  fileSize?: number
  startTime: string
  endTime: string
  secrecy?: number
  type?: string
  downloadUrl?: string
}
