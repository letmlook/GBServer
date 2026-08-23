import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export function startPlayback(deviceId: string, channelId: string, params: { startTime?: string; endTime?: string }) {
  return request<WvpResult<{ streamId: string; playUrl: string }>>({
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
  name: string
  filePath?: string
  startTime: string
  endTime: string
  secrecy?: number
  type?: string
}
