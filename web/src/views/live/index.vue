<template>
  <div class="live-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">实时直播</h1>
        <p class="page-subtitle">{{ stats.online }} 路在线 · {{ stats.total }} 路总计</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button @click="playDemoStream" plain :icon="VideoPlay">测试播放</el-button>
        <el-radio-group v-model="layout" size="small">
          <el-radio-button label="2x2">2×2</el-radio-button>
          <el-radio-button label="3x3">3×3</el-radio-button>
          <el-radio-button label="4x4">4×4</el-radio-button>
        </el-radio-group>
      </div>
    </div>

    <el-row :gutter="12">
      <el-col :xs="24" :md="6">
        <el-card class="device-tree-card">
          <template #header>
            <div class="card-header">
              <span>设备 / 通道</span>
              <el-input v-model="kw" size="small" placeholder="筛选" clearable style="width: 120px" />
            </div>
          </template>
          <el-tree
            v-if="tree.length > 0"
            :data="tree"
            :props="{ label: 'name', children: 'children' }"
            node-key="id"
            highlight-current
            :filter-node-method="filterNode"
            :default-expand-all="true"
            @node-click="onNodeClick"
            ref="treeRef"
            style="background: transparent"
          >
            <template #default="{ node, data }">
              <span class="tree-row">
                <span class="tree-label">{{ node.label }}</span>
                <el-tag
                  v-if="data.status"
                  :type="data.status === 'ON' ? 'success' : 'info'"
                  size="small"
                >
                  {{ data.status === 'ON' ? 'ON' : 'OFF' }}
                </el-tag>
              </span>
            </template>
          </el-tree>
          <el-empty v-else description="暂无通道；请先到「通道」页面确认 GB28181 设备已注册" :image-size="60" />
        </el-card>
      </el-col>

      <el-col :xs="24" :md="18">
        <el-card class="grid-card" v-loading="loading">
          <div v-if="!currentChannel" class="empty">
            <el-empty description="请从左侧选择通道开始播放，或点击右上「测试播放」验证播放器" />
          </div>
          <div v-else>
            <div class="player-bar">
              <span class="player-title">{{ currentChannel.name }}</span>
              <span class="player-meta mono small">
                {{ currentChannel.deviceId }} / {{ currentChannel.channelId }}
              </span>
              <el-tag
                :type="playerStatus === 'playing' ? 'success' : playerStatus === 'error' ? 'danger' : 'info'"
                size="small"
              >
                {{ statusLabel }}
              </el-tag>
            </div>

            <div :class="['video-grid', `video-grid--${layout}`]">
              <div
                v-for="(cell, idx) in cells"
                :key="idx"
                class="video-cell"
                :class="{ 'is-primary': cell.primary }"
              >
                <div class="video-cell__header">
                  <span class="video-cell__no">{{ String(idx + 1).padStart(2, '0') }}</span>
                  <span class="video-cell__title">{{ cell.name }}</span>
                  <el-tag v-if="cell.primary" type="danger" size="small" effect="dark">● LIVE</el-tag>
                </div>
                <div class="video-cell__body">
                  <!-- 浏览器原生 HLS（Safari） -->
                  <video
                    v-if="cell.primary && cell.url"
                    ref="primaryVideoRef"
                    :src="cell.url"
                    controls
                    autoplay
                    muted
                    playsinline
                    class="video-element native-hls"
                    style="display:none"
                    @error="onVideoError"
                    @loadeddata="onVideoLoaded"
                  />
                  <!-- flv.js（Chrome/Firefox） -->
                  <video
                    v-if="cell.primary && cell.url"
                    ref="flvVideoRef"
                    controls
                    autoplay
                    muted
                    playsinline
                    class="video-element flv-js"
                    style="display:none"
                  />
                  <div v-else class="video-placeholder">
                    <el-icon size="32"><VideoCameraFilled /></el-icon>
                    <p class="placeholder-tip">点击通道或「测试播放」开始</p>
                  </div>
                </div>
                <div class="video-cell__footer">
                  <span class="mono small">{{ cell.deviceId ?? '-' }} / {{ cell.channelId ?? '-' }}</span>
                  <el-button-group size="small">
                    <el-button @click="onSnap(cell)">抓图</el-button>
                    <el-button @click="onStop(cell)" type="danger" plain>停止</el-button>
                  </el-button-group>
                </div>
              </div>
            </div>

            <div v-if="playError" class="play-error">
              <el-alert :title="playError" type="warning" show-icon :closable="false" />
            </div>

            <div class="ptz-bar">
              <span class="ptz-title">PTZ:</span>
              <el-button-group>
                <el-button :icon="ArrowUp" @click="sendPtz(currentChannel, 'UP')" />
                <el-button :icon="ArrowLeft" @click="sendPtz(currentChannel, 'LEFT')" />
                <el-button :icon="VideoPause" @click="sendPtz(currentChannel, 'STOP')">停止</el-button>
                <el-button :icon="ArrowRight" @click="sendPtz(currentChannel, 'RIGHT')" />
                <el-button :icon="ArrowDown" @click="sendPtz(currentChannel, 'DOWN')" />
              </el-button-group>
              <el-button-group style="margin-left: 12px">
                <el-button @click="sendPtz(currentChannel, 'ZOOM_IN')">放大</el-button>
                <el-button @click="sendPtz(currentChannel, 'ZOOM_OUT')">缩小</el-button>
              </el-button-group>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import {
  ArrowUp,
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  VideoPause,
  VideoCameraFilled,
  VideoPlay
} from '@element-plus/icons-vue'
import {
  getPlayUrl,
  playSnap,
  sendPtz as sendPtzApi,
} from '@/api/live'
import { cameraListWithChild } from '@/api/syCamera'

const route = useRoute()
const router = useRouter()
const loading = ref(false)
const kw = ref('')
const layout = ref<'2x2' | '3x3' | '4x4'>('2x2')
const tree = ref<any[]>([])
const treeRef = ref<any>()
const channels = ref<{ deviceId: string; channelId: string; name: string; status: string; online: boolean }[]>([])
const currentChannel = ref<{ deviceId: string; channelId: string; name: string } | null>(null)
const cells = ref<any[]>([])
const primaryVideoRef = ref<HTMLVideoElement | null>(null)
const flvVideoRef = ref<HTMLVideoElement | null>(null)
const playError = ref<string>('')
const playerStatus = ref<'idle' | 'loading' | 'playing' | 'error'>('idle')

// 适配器实例
let hlsInstance: any = null
let flvPlayer: any = null

const stats = computed(() => ({
  total: channels.value.length,
  online: channels.value.filter((c) => c.online).length
}))

const statusLabel = computed(() => {
  switch (playerStatus.value) {
    case 'loading': return '加载中'
    case 'playing': return '● 播放中'
    case 'error':   return '播放失败'
    default:        return '空闲'
  }
})

watch(kw, (v) => treeRef.value?.filter(v))

function filterNode(value: string, data: any) {
  if (!value) return true
  return data.name?.includes(value) ?? false
}

async function loadData() {
  loading.value = true
  try {
    const res = await cameraListWithChild({ page: 1, count: 1000 })
    const list = res.data?.list ?? []
    channels.value = list
      .filter((c: any) => c.channel_id)
      .map((c: any) => ({
        deviceId: c.device_id,
        channelId: c.channel_id,
        name: c.name ?? c.channel_id,
        status: c.status,
        online: !!c.online,
      }))

    // 构建设备树
    const grouped = new Map<string, any[]>()
    for (const ch of channels.value) {
      const arr = grouped.get(ch.deviceId) ?? []
      arr.push({ id: ch.channelId, name: ch.name, status: ch.status, raw: ch })
      grouped.set(ch.deviceId, arr)
    }
    tree.value = Array.from(grouped.entries()).map(([deviceId, children]) => {
      const devName = channels.value.find((c) => c.deviceId === deviceId)?.name ?? deviceId
      return { id: deviceId, name: devName, children }
    })
  } catch (e: any) {
    ElMessage.error(e?.message ?? '通道列表加载失败')
    tree.value = []
  } finally {
    loading.value = false
  }
}

async function onNodeClick(node: any) {
  if (!node?.raw) return
  await playChannel(node.raw)
}

async function playChannel(s: { deviceId: string; channelId: string; name?: string }) {
  playError.value = ''
  playerStatus.value = 'loading'
  try {
    // 优先尝试 HLS（浏览器 + hls.js）
    const res = await getPlayUrl({ deviceId: s.deviceId, channelId: s.channelId, protocol: 'hls' })
    const url = res.data?.url ?? ''
    if (!url) {
      // 回退到 RTSP / FLV
      const rtspRes = await getPlayUrl({ deviceId: s.deviceId, channelId: s.channelId, protocol: 'rtsp' }).catch(() => null)
      const fallback = rtspRes?.data?.url
      if (!fallback) {
        playError.value = `通道 ${s.channelId} 暂无可用播放地址（确认 GBServer 已注册 + ZLM 已上线 + HLS 启用）`
        playerStatus.value = 'error'
        currentChannel.value = { deviceId: s.deviceId, channelId: s.channelId, name: s.name ?? s.channelId }
        buildGrid({ ...s, name: s.name ?? s.channelId }, '')
        return
      }
      currentChannel.value = { deviceId: s.deviceId, channelId: s.channelId, name: s.name ?? s.channelId }
      buildGrid({ ...s, name: s.name ?? s.channelId }, fallback)
      await nextTick()
      await attachVideo(fallback)
      return
    }
    currentChannel.value = { deviceId: s.deviceId, channelId: s.channelId, name: s.name ?? s.channelId }
    buildGrid({ ...s, name: s.name ?? s.channelId }, url)
    await nextTick()
    await attachVideo(url)
  } catch (e: any) {
    playError.value = e?.message ?? '获取播放地址失败'
    playerStatus.value = 'error'
    ElMessage.error(playError.value)
  }
}

async function attachVideo(url: string) {
  // 清理之前的实例
  hlsInstance?.destroy?.()
  hlsInstance = null
  if (flvPlayer) {
    try { flvPlayer.destroy() } catch {}
    flvPlayer = null
  }
  const nativeVideo = primaryVideoRef.value
  const flvVideo = flvVideoRef.value
  if (!nativeVideo || !url) return

  // Safari 原生 HLS
  const canNativeHls = nativeVideo.canPlayType('application/vnd.apple.mpegurl') !== ''
  if (canNativeHls) {
    nativeVideo.style.display = 'block'
    if (flvVideo) flvVideo.style.display = 'none'
    nativeVideo.src = url
    return
  }

  // Chrome/Firefox：先尝试 hls.js（HLS），失败则 flv.js（FLV）
  const Hls = await loadScript('hls.js', [
    'https://cdn.jsdelivr.net/npm/hls.js@1.5.13/dist/hls.min.js',
    'https://unpkg.com/hls.js@1.5.13/dist/hls.min.js',
  ])
  if (Hls && Hls.isSupported()) {
    nativeVideo.style.display = 'block'
    if (flvVideo) flvVideo.style.display = 'none'
    const hls = new Hls({ liveSyncDuration: 1, enableWorker: true })
    hls.loadSource(url)
    hls.attachMedia(nativeVideo)
    hls.on(Hls.Events.ERROR, (_e: any, data: any) => {
      if (data?.fatal) {
        playError.value = `HLS 错误: ${data?.details ?? data?.type ?? 'unknown'}（确认 ZLM HLS 已启用或使用 FLV）`
        playerStatus.value = 'error'
      }
    })
    hlsInstance = hls
    return
  }

  // HLS 不支持或 hls.js 加载失败，尝试 flv.js
  const flvjs = await loadScript('flv.js', [
    'https://cdn.jsdelivr.net/npm/flv.js@1.6.2/dist/flv.min.js',
    'https://unpkg.com/flv.js@1.6.2/dist/flv.min.js',
  ])
  if (flvjs && flvjs.isSupported() && flvVideo) {
    nativeVideo.style.display = 'none'
    flvVideo.style.display = 'block'
    const player = flvjs.createPlayer({
      type: 'flv',
      url,
      isLive: true,
    })
    player.attachMediaElement(flvVideo)
    try { player.load() } catch (e) { /* ignore */ }
    player.play().catch((e: any) => {
      playError.value = `FLV 播放失败: ${e?.message ?? e}`
      playerStatus.value = 'error'
    })
    player.on(flvjs.Events.ERROR, (errType: string, errDetail: string) => {
      playError.value = `FLV 错误: ${errType} / ${errDetail}`
      playerStatus.value = 'error'
    })
    flvPlayer = player
    return
  }

  // 最后尝试：直接用原生 <video> src（可能能播 RTMP 之外的格式）
  nativeVideo.style.display = 'block'
  if (flvVideo) flvVideo.style.display = 'none'
  nativeVideo.src = url
  playError.value = '浏览器不支持该视频格式；建议使用 Chrome / Safari / Edge 访问'
}

function loadScript(globalName: string, cdnUrls: string[]): Promise<any> {
  return new Promise((resolve) => {
    const w = window as any
    if (w[globalName]) return resolve(w[globalName])
    let idx = 0
    const tryLoad = () => {
      if (idx >= cdnUrls.length) return resolve(null)
      const s = document.createElement('script')
      s.src = cdnUrls[idx++]
      s.async = true
      s.onload = () => resolve(w[globalName] ?? null)
      s.onerror = () => tryLoad()
      document.head.appendChild(s)
    }
    tryLoad()
  })
}

function onVideoLoaded() {
  playerStatus.value = 'playing'
  playError.value = ''
}

function onVideoError(_e: Event) {
  playerStatus.value = 'error'
  if (!playError.value) {
    playError.value = '视频流加载失败（确认 ZLM 已开启 HLS 或 FLV，且 stream_id 与 URL 一致）'
  }
}

function buildGrid(s: any, url: string) {
  const count = layout.value === '2x2' ? 4 : layout.value === '3x3' ? 9 : 16
  const grid: any[] = []
  grid.push({ ...s, url, primary: true })
  while (grid.length < count) grid.push({})
  cells.value = grid
}

async function onSnap(cell: any) {
  if (!cell?.deviceId || !cell?.channelId) return
  try {
    await playSnap(cell.deviceId, cell.channelId)
    ElMessage.success('抓图已保存')
  } catch (e: any) {
    ElMessage.error(e?.message ?? '抓图失败')
  }
}

async function onStop(cell: any) {
  hlsInstance?.destroy?.()
  hlsInstance = null
  if (flvPlayer) { try { flvPlayer.destroy() } catch {} flvPlayer = null }
  if (primaryVideoRef.value) { primaryVideoRef.value.src = ''; primaryVideoRef.value.style.display = 'none' }
  if (flvVideoRef.value) { flvVideoRef.value.src = ''; flvVideoRef.value.style.display = 'none' }
  buildGrid(cell, '')
  playerStatus.value = 'idle'
  ElMessage.success('已停止')
}

async function sendPtz(channel: any, cmd: string) {
  if (!channel?.deviceId || !channel?.channelId) {
    ElMessage.warning('请先选择播放通道')
    return
  }
  try {
    await sendPtzApi({ deviceId: channel.deviceId, channelId: channel.channelId, cmd })
    ElMessage.success(`PTZ ${cmd} 已下发`)
  } catch (e: any) {
    ElMessage.error(e?.message ?? `PTZ ${cmd} 失败`)
  }
}

/**
 * 播放公共测试流（不依赖 ZLM / SIP / 任何真实环境）
 * 用于验证浏览器播放器是否正常工作
 */
async function playDemoStream() {
  // 多个公共测试 HLS 流，按顺序尝试
  const demoStreams = [
    { url: 'https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8', name: 'Mux 测试 HLS' },
    { url: 'https://demo.unified-streaming.com/k8s/features/stable/video/tears-of-steel/tears-of-steel.ism/.m3u8', name: 'Unified 测试 HLS' },
    { url: 'https://flv.bn.nflxvideo.net/4b91eae.mp4', name: 'Big Buck Bunny (mp4)' },
  ]
  const first = demoStreams[0]
  currentChannel.value = { deviceId: 'DEMO', channelId: 'DEMO', name: first.name }
  buildGrid({ deviceId: 'DEMO', channelId: 'DEMO', name: first.name }, first.url)
  await nextTick()
  playerStatus.value = 'loading'
  await attachVideo(first.url)
  ElMessage.info(`播放公共测试流：${first.name}（如失败请尝试下一个）`)
}

onBeforeUnmount(() => {
  hlsInstance?.destroy?.()
  if (flvPlayer) { try { flvPlayer.destroy() } catch {} }
})

onMounted(async () => {
  await loadData()
  const qd = route.query.deviceId as string | undefined
  const qc = route.query.channelId as string | undefined
  if (qd && qc) {
    await nextTick()
    const target = channels.value.find((c) => c.deviceId === qd && c.channelId === qc)
    if (target) await playChannel(target)
  }
})
</script>

<style scoped>
.live-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.device-tree-card { height: calc(100vh - 200px); overflow: auto; }
.grid-card { min-height: 600px; }
.card-header { display: flex; justify-content: space-between; align-items: center; }
.empty { padding: 80px 0; }
.tree-row { display: flex; justify-content: space-between; align-items: center; gap: 8px; width: 100%; }
.tree-label { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.player-bar { display: flex; align-items: center; gap: 12px; padding: 8px 12px; background: #f7f7f7; border-radius: 4px; margin-bottom: 8px; }
.player-title { font-weight: 600; }
.player-meta { color: var(--el-text-color-secondary); }
.video-grid { display: grid; gap: 6px; }
.video-grid--2x2 { grid-template-columns: repeat(2, 1fr); }
.video-grid--3x3 { grid-template-columns: repeat(3, 1fr); }
.video-grid--4x4 { grid-template-columns: repeat(4, 1fr); }
.video-cell { background: #0b0b0b; color: #fff; border-radius: 6px; overflow: hidden; aspect-ratio: 16/9; position: relative; display: flex; flex-direction: column; }
.video-cell.is-primary { box-shadow: 0 0 0 2px var(--el-color-danger); }
.video-cell__header { display: flex; gap: 8px; align-items: center; padding: 6px 10px; background: rgba(0,0,0,.6); font-size: 12px; }
.video-cell__no { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; opacity: 0.7; }
.video-cell__title { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.video-cell__body { flex: 1; display: flex; align-items: center; justify-content: center; background: #111; position: relative; }
.video-cell__footer { display: flex; justify-content: space-between; align-items: center; padding: 4px 10px; background: rgba(0,0,0,.6); font-size: 11px; }
.video-element { width: 100%; height: 100%; object-fit: contain; background: #000; }
.video-placeholder { color: #555; text-align: center; }
.placeholder-tip { font-size: 12px; margin-top: 4px; opacity: 0.6; }
.ptz-bar { padding: 12px; display: flex; align-items: center; }
.ptz-title { margin-right: 8px; color: var(--el-text-color-secondary); }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
.small { font-size: 11px; }
.play-error { padding: 0 16px 8px; }
</style>