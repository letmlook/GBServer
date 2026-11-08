import { request } from '@/utils/request'
import type { ApiResult } from '@/types/api'

export interface PlaybackRecoveryItem {
  streamId: string
  deviceId: string
  channelId: string
  callId: string | null
  status: 'cleanup_pending'
  deviceResult: 'unconfirmed'
}

export function listPlaybackRecovery(offset: number, limit: number) {
  return request<ApiResult<{ items: PlaybackRecoveryItem[]; total: number; offset: number; limit: number }>>({
    method: 'get', url: '/playback/recovery', params: { offset, limit }
  })
}

export function retryPlaybackRecovery(streamId: string) {
  return request<ApiResult<{ status: string; deviceResult: string; message: string }>>({
    method: 'post', url: `/playback/recovery/${encodeURIComponent(streamId)}/retry`
  })
}
