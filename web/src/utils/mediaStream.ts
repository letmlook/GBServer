import type { MediaStreamRow } from '@/api/live'

/**
 * ZLM 流列表（`/api/device/query/streams`）的两个"坑"以及对应的收敛函数。
 *
 * 坑 1：ZLM `getMediaList` 对**同一路流按协议各返回一行** —— 实测 ZLM master 上，
 * 一路 `rtp/34020000001320000001_34020000001320000001` 会返回
 * `rtsp` / `rtmp` / `hls` / `ts` / `fmp4` **五行**。所以 `list.length` 是「行数」，
 * 不是「路数」：直接拿去计数会把 1 路流数成 5 路（控制台「活跃通道」卡片右侧的
 * 「直播 N」就曾经这么虚高）。
 *
 * 坑 2：同一路流在不同协议行上的 `readerCount` 不一致（读者只挂在真正被播放的
 * 那个协议行上），所以判断"有没有人在看"必须取各行的最大值。
 *
 * 更深一层的坑：一个**通道**可能同时有多路流（实时 `设备ID_通道ID` + 回放/下载
 * `设备ID_通道ID_开始_结束`）。凡是"通道"口径的界面（控制台「重点通道」）必须再
 * 按通道归并一次，否则同一通道会占掉多个格子 —— 即用户报的
 * 「重要通道里面相同的通道视频会出现多个」。
 */

/** 一路流的身份：同一节点上由 (mediaServerId, app, stream) 唯一确定。 */
export function streamSourceKey(row: MediaStreamRow): string {
  return `${row.mediaServerId ?? ''}|${row.app ?? ''}|${row.stream ?? ''}`
}

/**
 * 把 ZLM 原始行收敛成「一路流一行」。
 *
 * 合并同源的多个协议行，读者数 / 存活时长 / 码率取最大值（读者数只挂在被真正
 * 播放的那个协议行上，取首个会恒为 0）。
 */
export function dedupeMediaStreams(rows: MediaStreamRow[]): MediaStreamRow[] {
  const bySource = new Map<string, MediaStreamRow>()
  for (const row of rows) {
    const key = streamSourceKey(row)
    const kept = bySource.get(key)
    if (!kept) {
      bySource.set(key, { ...row })
      continue
    }
    kept.readerCount = Math.max(kept.readerCount ?? 0, row.readerCount ?? 0)
    kept.totalReaderCount = Math.max(kept.totalReaderCount ?? 0, row.totalReaderCount ?? 0)
    kept.aliveSecond = Math.max(kept.aliveSecond ?? 0, row.aliveSecond ?? 0)
    kept.bytesSpeed = Math.max(kept.bytesSpeed ?? 0, row.bytesSpeed ?? 0)
  }
  return [...bySource.values()]
}

/**
 * 是否是**实时**流。实时流名就是 `设备ID_通道ID`（后端从流名前两段解析出
 * deviceId/channelId，故这里用同样的口径回比）；回放/下载流名会在后面再挂
 * `_开始时间_结束时间`。
 */
export function isLiveStream(row: MediaStreamRow): boolean {
  const deviceId = row.deviceId ?? ''
  const channelId = row.channelId ?? ''
  if (!deviceId || !channelId) return false
  return row.stream === `${deviceId}_${channelId}`
}

/** 是否为可点播的国标通道流（推流 `push_*` / 代理 `proxy_*` 没有国标标识）。 */
export function isGbChannelStream(row: MediaStreamRow): boolean {
  return !!(row.deviceId && row.channelId)
}

/** 「重点通道」面板的一个格子 */
export interface KeyChannel {
  /** 稳定标识 `设备ID_通道ID` —— 作为 v-for 的 key，避免每次轮询重挂 DOM */
  key: string
  deviceId: string
  channelId: string
  /** 代表流名：同一通道优先取实时流 */
  stream: string
  /** 该通道当前是否有实时流（false = 只有回放/下载流在跑） */
  live: boolean
  /** 该通道所有流里的最大读者数 */
  readerCount: number
}

/**
 * 从 ZLM 原始行里挑出「重点通道」：按通道去重，最多 `limit` 个。
 *
 * 规则：
 *  1. 先按 (节点, app, 流名) 去重，消掉协议行的倍数；
 *  2. 再按 `设备ID_通道ID` 归并 —— 同一通道的实时流与回放流只占**一个**格子，
 *     优先保留实时流（面板点击是实时预览）；
 *  3. 排序：实时流在前，其次读者多的在前，最后按通道号稳定排序
 *     （面板每 2s 轮询一次，顺序必须稳定，否则格子会来回跳）。
 */
export function extractKeyChannels(rows: MediaStreamRow[], limit = 6): KeyChannel[] {
  const byChannel = new Map<string, KeyChannel>()
  for (const row of dedupeMediaStreams(rows)) {
    if (!isGbChannelStream(row)) continue
    const deviceId = row.deviceId
    const channelId = row.channelId
    const key = `${deviceId}_${channelId}`
    const live = isLiveStream(row)
    const readerCount = row.readerCount ?? 0
    const prev = byChannel.get(key)
    if (!prev) {
      byChannel.set(key, { key, deviceId, channelId, stream: row.stream, live, readerCount })
      continue
    }
    // 代表流名：实时流优先；同为实时（或同为回放）时选读者多的那一路。
    // 比较要在更新 readerCount **之前**做，否则"当前行读者最多"会被自己抹平。
    const replace = prev.live === live ? readerCount > prev.readerCount : live
    if (replace) {
      prev.stream = row.stream
      prev.live = live
    }
    prev.readerCount = Math.max(prev.readerCount, readerCount)
  }

  return [...byChannel.values()]
    .sort((a, b) => {
      if (a.live !== b.live) return a.live ? -1 : 1
      if (a.readerCount !== b.readerCount) return b.readerCount - a.readerCount
      return a.key < b.key ? -1 : a.key > b.key ? 1 : 0
    })
    .slice(0, limit)
}
