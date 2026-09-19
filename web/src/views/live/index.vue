<template>
  <div class="live-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">实时直播</h1>
        <p class="page-subtitle">{{ stats.online }} 路在线 · {{ stats.total }} 路总计</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <!-- 布局切换：与「刷新」同款普通按钮（默认 size），仅用 type 区分选中态。
             之前是 el-radio-group size="small"，比刷新按钮小一圈、样式也不同套。 -->
        <el-button
          v-for="opt in layoutOptions"
          :key="opt.value"
          :type="layout === opt.value ? 'primary' : 'default'"
          @click="layout = opt.value"
        >
          {{ opt.label }}
        </el-button>
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
                <!-- 设备节点用图标 + 加粗区分；通道节点用普通文字 -->
                <el-icon v-if="data.isDevice" class="tree-icon"><Grid /></el-icon>
                <el-icon v-else class="tree-icon tree-icon--channel"><VideoCamera /></el-icon>
                <span :class="['tree-label', { 'tree-label--device': data.isDevice }]">
                  {{ node.label }}
                </span>
                <!-- 设备节点显示通道数；通道节点显示在线状态 -->
                <span v-if="data.isDevice" class="tree-count">{{ (data.children ?? []).length }}</span>
                <el-tag
                  v-else-if="data.status"
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
            <el-empty description="请从左侧选择通道开始播放" />
          </div>
          <div v-else>
            <!-- 通道信息条：只显示通道名 / 国标 ID / 播放状态。
                 云台与对讲已从本页移除 —— 云台在「通道播放」对话框里提供。 -->
            <div class="player-bar">
              <div class="player-bar__left">
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
                    <p class="placeholder-tip">点击通道开始</p>
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
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 抓图预览：成功后在右下角弹出最近一张抓图，点击放大；多张累积 -->
    <SnapPreview
      v-model="snapVisible"
      :items="snapItems"
      @clear="snapItems = []"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import {
  Grid,
  VideoCamera,
  VideoCameraFilled
} from '@element-plus/icons-vue'
import {
  captureSnap,
  listSnapshots,
  postWebrtcPlay,
  snapshotKey,
  startPlay,
  stopPlay,
} from '@/api/live'
import { cameraListWithChild } from '@/api/syCamera'
import SnapPreview from '@/components/SnapPreview/index.vue'

const route = useRoute()
const router = useRouter()
const loading = ref(false)
const kw = ref('')
// 默认单画面：多数场景是"选一路看"，1×1 让画面最大化；
// 需要多路时用户再点 2×2 / 3×3 / 4×4。
const layout = ref<'1x1' | '2x2' | '3x3' | '4x4'>('1x1')
const layoutOptions = [
  { value: '1x1' as const, label: '1×1' },
  { value: '2x2' as const, label: '2×2' },
  { value: '3x3' as const, label: '3×3' },
  { value: '4x4' as const, label: '4×4' }
]
const tree = ref<any[]>([])
const treeRef = ref<any>()
const channels = ref<{ deviceId: string; channelId: string; name: string; status: string; online: boolean }[]>([])
const currentChannel = ref<{ deviceId: string; channelId: string; name: string } | null>(null)
const cells = ref<any[]>([])
const primaryVideoRef = ref<HTMLVideoElement | null>(null)
const flvVideoRef = ref<HTMLVideoElement | null>(null)
const playError = ref('')
const playerStatus = ref<'idle' | 'loading' | 'playing' | 'error'>('idle')

// 抓图预览状态：抓图成功累积到列表里（最多保留 8 张），右下角浮窗可点开
const snapVisible = ref(false)
const snapItems = ref<{ deviceId: string; channelId: string; name: string; snapUrl: string; time: number }[]>([])

// 适配器实例
let hlsInstance: any = null
let flvPlayer: any = null
let webrtcPc: RTCPeerConnection | null = null

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
// 切网格布局时重建格子（不要重新拉流，url 已经在当前主格子里）
watch(layout, () => {
  if (!currentChannel.value) return
  const url = (cells.value.find((c) => c.primary)?.url) ?? ''
  buildGrid(currentChannel.value, url)
})

function filterNode(value: string, data: any) {
  if (!value) return true
  return data.name?.includes(value) ?? false
}

async function loadData() {
  loading.value = true
  try {
    const res = await cameraListWithChild({ page: 1, count: 1000 })
    const list = (res.data?.list ?? []) as any[]

    // 通道行（`is_device=false`）才是真正可点播的通道；
    // 每行都带 `device_name`（后端 CameraRow.device_name），可直接当树的父节点名。
    const channelRows = list.filter((c: any) => c.channel_id && !c.is_device)
    channels.value = channelRows.map((c: any) => ({
      deviceId: c.device_id,
      channelId: c.channel_id,
      name: c.name ?? c.channel_id,
      status: c.status,
      online: !!c.online,
    }))

    // 设备元信息：device_id → { name, status }。
    // 设备名优先取通道行携带的 `device_name`；
    // 对"无通道的设备"（is_device=true 的行），它自己的 name 就是设备名。
    const deviceMeta = new Map<string, { name: string; status: string; online: boolean }>()
    for (const r of list) {
      const devId = r.device_id
      if (!devId) continue
      const candidate =
        (r.device_name && String(r.device_name).trim()) ||
        (r.is_device ? String(r.name ?? '').trim() : '') ||
        devId
      // 首次写入优先（同一个 device_id 的多行 device_name 相同，幂等）
      if (!deviceMeta.has(devId)) {
        deviceMeta.set(devId, {
          name: candidate,
          status: r.status,
          online: !!r.online
        })
      }
    }

    // 构建设备 / 通道两级树：父节点 = 设备，子节点 = 该设备下的通道
    const grouped = new Map<string, any[]>()
    for (const ch of channels.value) {
      const arr = grouped.get(ch.deviceId) ?? []
      arr.push({ id: ch.channelId, name: ch.name, status: ch.status, raw: ch })
      grouped.set(ch.deviceId, arr)
    }
    // 无通道的设备也保留在树里（展开后为空），避免设备从预览页消失
    for (const devId of deviceMeta.keys()) {
      if (!grouped.has(devId)) grouped.set(devId, [])
    }

    tree.value = Array.from(grouped.entries())
      .map(([deviceId, children]) => {
        const meta = deviceMeta.get(deviceId)
        return {
          id: deviceId,
          name: meta?.name ?? deviceId,
          // 标记为"设备节点"：模板据此换图标、加粗、显示通道数、不响应点播
          isDevice: true,
          // 设备级在线状态
          status: meta?.status,
          online: meta?.online,
          children
        }
      })
      // 设备按名字排序，便于快速定位
      .sort((a, b) => a.name.localeCompare(b.name, 'zh-Hans-CN'))
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
  const label = { deviceId: s.deviceId, channelId: s.channelId, name: s.name ?? s.channelId }
  try {
    // 关键：先拉起流（后端发 SIP INVITE + 开 ZLM RTP server），再拿它返回的
    // 播放地址。此前这里直接调 getPlayUrl 拼地址 —— **从未拉起流**，
    // 于是返回的是一个不存在的东西的地址，画面永远出不来。
    if (currentChannel.value?.channelId && currentChannel.value.channelId !== s.channelId) {
      // 切换通道时先停掉上一路，避免设备侧与 ZLM 侧残留
      await stopPlay(currentChannel.value.deviceId, currentChannel.value.channelId).catch(() => {})
    }
    const res = await startPlay(s.deviceId, s.channelId)
    const data = res.data as any
    if (!data) {
      throw new Error(res.msg || '拉起实时流失败')
    }
    // 优先 WebRTC（最低延迟），失败回退到 HLS / FLV。
    // WebRTC 不需要 URL —— 直接拿 deviceId+channelId 做 SDP 协商。
    currentChannel.value = label
    buildGrid({ ...s, name: label.name }, data.hls || data.flvUrl || data.playUrl || '')
    await nextTick()
    if (data.webrtc) {
      try {
        await attachVideoWebRTC(s.deviceId, s.channelId)
        return
      } catch (e: any) {
        // WebRTC 协商失败时清理回退，再走 HLS/FLV 路径
        const msg = e?.message ?? String(e)
        playError.value = `WebRTC 协商失败，回退到 HLS/FLV：${msg}`
        cleanupWebRTC()
      }
    }
    const url = data.hls || data.flvUrl || data.playUrl || ''
    if (!url) {
      throw new Error('后端未返回可用的播放地址')
    }
    await attachVideo(url)
  } catch (e: any) {
    const msg = e?.message ?? '拉起实时流失败'
    playError.value = msg
    playerStatus.value = 'error'
    ElMessage.error(`通道 ${s.channelId} 播放失败：${msg}`)
  }
}

async function attachVideoWebRTC(deviceId: string, channelId: string) {
  // 关掉任何残留
  cleanupWebRTC()
  hlsInstance?.destroy?.()
  hlsInstance = null
  if (flvPlayer) { try { flvPlayer.destroy() } catch {} flvPlayer = null }

  // 同子网直连：host 网络下 ZLM ICE 候选里有 192.168.3.87，浏览器也在
  // 192.168.3.x，能直接 host candidate 联通，**不需要 STUN**。空 iceServers
  // 比错的 STUN URL 强（错的 STUN 会让浏览器忽略 host candidate 走 relay）。
  const pc = new RTCPeerConnection({ iceServers: [] })

  pc.addTransceiver('video', { direction: 'recvonly' })
  pc.addTransceiver('audio', { direction: 'recvonly' })

  // ref 在 v-for 里是数组，先拍平
  const nativeVideo = (Array.isArray(primaryVideoRef.value)
    ? primaryVideoRef.value.find((v: any) => v)
    : primaryVideoRef.value) as HTMLVideoElement | null
  if (!nativeVideo) throw new Error('video 元素未挂载')

  // 视频流拿到就喂进 srcObject
  pc.ontrack = (ev) => {
    if (ev.streams && ev.streams[0] && nativeVideo) {
      nativeVideo.srcObject = ev.streams[0]
      nativeVideo.style.display = 'block'
      nativeVideo.muted = true
      nativeVideo.play().catch(() => { /* 静音自动播放被浏览器拒，没事 */ })
      playerStatus.value = 'playing'
      playError.value = ''
    }
  }
  pc.oniceconnectionstatechange = () => {
    if (pc.iceConnectionState === 'failed' || pc.iceConnectionState === 'disconnected') {
      playerStatus.value = 'error'
      playError.value = `WebRTC ICE ${pc.iceConnectionState}（ZLM ${deviceId}/${channelId} 的 rtc 端口未开放或 ICE 候选不通）`
    }
  }

  // SDP 协商
  const offer = await pc.createOffer()
  await pc.setLocalDescription(offer)

  const res = await postWebrtcPlay({
    deviceId,
    channelId,
    sdp: offer.sdp ?? '',
    type: 'offer'
  })
  if (res.code !== 0) {
    pc.close()
    throw new Error(res.msg || 'WebRTC 协商失败')
  }
  await pc.setRemoteDescription({ type: 'answer', sdp: res.data.sdp })

  webrtcPc = pc
}

function cleanupWebRTC() {
  if (webrtcPc) {
    try { webrtcPc.close() } catch { /* ignore */ }
    webrtcPc = null
  }
  const nativeVideo = Array.isArray(primaryVideoRef.value)
    ? primaryVideoRef.value.find((v: any) => v)
    : primaryVideoRef.value
  if (nativeVideo) {
    try {
      const ms = (nativeVideo as HTMLVideoElement).srcObject as MediaStream | null
      if (ms) ms.getTracks().forEach((t) => t.stop())
    } catch { /* ignore */ }
    ;(nativeVideo as HTMLVideoElement).srcObject = null
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
  // ref 在 v-for 内时，Vue 会把它收成数组（即便只有一个元素挂上去）。
  // primaryVideoRef.value 在多 cell 布局下可能是 `[videoEl, null, ...]`，
  // 直接调 `.canPlayType()` 会抛 `e.canPlayType is not a function`——
  // 这就是历史 "播放失败" 的隐性 bug，统一拍平成单个元素。
  const nativeVideo = Array.isArray(primaryVideoRef.value)
    ? primaryVideoRef.value.find((v: any) => v) ?? null
    : primaryVideoRef.value
  const flvVideo = Array.isArray(flvVideoRef.value)
    ? flvVideoRef.value.find((v: any) => v) ?? null
    : flvVideoRef.value
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
  try {
    const Hls = (await import('hls.js')).default
    if (Hls.isSupported()) {
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
  } catch (e: any) {
    // hls.js 加载或初始化失败，回退到 flv.js
    playError.value = `hls.js 不可用: ${e?.message ?? e}`
  }

  // HLS 不支持或 hls.js 加载失败，尝试 flv.js
  try {
    const flvjs = (await import('flv.js')).default
    if (flvjs.isSupported() && flvVideo) {
      nativeVideo.style.display = 'none'
      flvVideo.style.display = 'block'
      const player = flvjs.createPlayer({
        type: 'flv',
        url,
        isLive: true,
      })
      player.attachMediaElement(flvVideo)
      try { player.load() } catch (e) { /* ignore */ }
      Promise.resolve(player.play()).catch((e: any) => {
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
  } catch (e: any) {
    playError.value = `flv.js 不可用: ${e?.message ?? e}`
  }

  // 最后尝试：直接用原生 <video> src（可能能播 RTMP 之外的格式）
  nativeVideo.style.display = 'block'
  if (flvVideo) flvVideo.style.display = 'none'
  nativeVideo.src = url
  playError.value = '浏览器不支持该视频格式；建议使用 Chrome / Safari / Edge 访问'
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
  // 1×1 单画面：只显示当前通道一个格子且占满
  const count = layout.value === '1x1' ? 1 : layout.value === '2x2' ? 4 : layout.value === '3x3' ? 9 : 16
  const grid: any[] = []
  grid.push({ ...s, url, primary: true })
  while (grid.length < count) grid.push({})
  cells.value = grid
}

/**
 * 「抓图」按钮：让后端立刻从该通道当前流里抓一帧、覆盖保存为缩略图，
 * 再取回新 URL 累积到预览浮窗。
 */
async function onSnap(cell: any) {
  if (!cell?.deviceId || !cell?.channelId) return
  const key = snapshotKey(cell.deviceId, cell.channelId)
  try {
    await captureSnap(cell.deviceId, cell.channelId)
    const res = await listSnapshots([key])
    const snapUrl = res?.data?.[key] ?? ''
    if (snapUrl) {
      // 累积到预览列表里（最近 8 张），让"抓了却看不到图"的体验变好
      snapItems.value.unshift({
        deviceId: cell.deviceId,
        channelId: cell.channelId,
        name: cell.name ?? cell.channelId,
        snapUrl,
        time: Date.now()
      })
      if (snapItems.value.length > 8) snapItems.value.length = 8
      snapVisible.value = true
    }
    ElMessage.success('抓图已保存')
  } catch (e: any) {
    ElMessage.error(e?.message ?? '抓图失败')
  }
}

async function onStop(cell: any) {
  hlsInstance?.destroy?.()
  hlsInstance = null
  if (flvPlayer) { try { flvPlayer.destroy() } catch {} flvPlayer = null }
  cleanupWebRTC()
  // ref 在 v-for 里同样是数组；与 attachVideo 一致地取真实 video 元素
  const nativeVideo = Array.isArray(primaryVideoRef.value)
    ? primaryVideoRef.value.find((v: any) => v)
    : primaryVideoRef.value
  const flvVideo = Array.isArray(flvVideoRef.value)
    ? flvVideoRef.value.find((v: any) => v)
    : flvVideoRef.value
  if (nativeVideo) { nativeVideo.src = ''; nativeVideo.style.display = 'none' }
  if (flvVideo) { flvVideo.src = ''; flvVideo.style.display = 'none' }
  // 真正停流：后端会发 SIP BYE 并关闭 ZLM RTP server / 收流
  const target = cell?.deviceId && cell?.channelId ? cell : currentChannel.value
  if (target?.deviceId && target?.channelId) {
    await stopPlay(target.deviceId, target.channelId).catch((e) => {
      ElMessage.warning(`停止流失败：${e instanceof Error ? e.message : String(e)}`)
    })
  }
  buildGrid(cell, '')
  playerStatus.value = 'idle'
  ElMessage.success('已停止')
}

onBeforeUnmount(() => {
  hlsInstance?.destroy?.()
  if (flvPlayer) { try { flvPlayer.destroy() } catch {} }
  cleanupWebRTC()
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
/* 整体约束：直播页要"塞进一屏"。左侧设备树 + 右侧播放卡都不能撑破视口，
   否则底部区域被滚出屏幕。 */
.live-page {
  padding: 16px;
  display: flex;
  flex-direction: column;
  /* 顶部栏 56px + 上下 padding 32px + 头部区 ~60px ≈ 150px 留出；
     ElRow 内部继续 flex 1 把剩余空间分给视频和设备列表。 */
  height: calc(100vh - 16px);
  box-sizing: border-box;
}
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; flex: 0 0 auto; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: var(--text-sm); margin-top: 4px; }

/* 主区：左侧设备栏 + 右侧播放卡 占满剩余高度；
   关键是不让任一列把页面撑高导致整体出现竖向滚动条。 */
.live-page :deep(.el-row),
.live-page :deep(.el-row > .el-col) {
  display: flex;
}
.live-page :deep(.el-row > .el-col) { min-height: 0; }

.device-tree-card { flex: 1; overflow: auto; }
.grid-card {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0; /* flex 子项允许收缩 */
}
/* 播放卡内部：信息条固定、视频区弹性占满剩余高度。 */
.grid-card :deep(.el-card__body) {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.card-header { display: flex; justify-content: space-between; align-items: center; }
.empty { padding: 80px 0; }
.tree-row { display: flex; align-items: center; gap: 6px; width: 100%; padding-right: 4px; }
.tree-label { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
/* 设备节点（父）：加粗 + 深色，与通道子节点形成层级对比 */
.tree-label--device { font-weight: 600; color: var(--text-primary); }
.tree-icon { color: var(--brand-primary-500); flex: 0 0 auto; }
.tree-icon--channel { color: var(--text-tertiary); }
/* 设备节点右侧的通道数角标 */
.tree-count {
  flex: 0 0 auto;
  min-width: 20px;
  padding: 0 6px;
  height: 18px;
  line-height: 18px;
  text-align: center;
  border-radius: 999px;
  background: var(--bg-elevated);
  color: var(--text-tertiary);
  font-size: 11px;
  font-weight: 600;
  font-family: var(--font-mono);
}
.player-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 12px;
  background: #f7f7f7;
  border-radius: 4px;
  margin-bottom: 8px;
  flex: 0 0 auto;
  flex-wrap: wrap;
}
.player-bar__left {
  display: flex; align-items: center; gap: 12px;
  min-width: 0;
  flex: 1 1 auto;
}
.player-title { font-weight: 600; }
.player-meta { color: var(--el-text-color-secondary); }

/* 视频区：吃掉播放卡除头尾外的所有空间，格子不再硬撑 480px。 */
.video-grid {
  display: grid;
  gap: 6px;
  flex: 1;
  min-height: 0;
}
.video-grid--1x1 { grid-template-columns: 1fr; }
.video-grid--2x2 { grid-template-columns: repeat(2, 1fr); }
.video-grid--3x3 { grid-template-columns: repeat(3, 1fr); }
.video-grid--4x4 { grid-template-columns: repeat(4, 1fr); }

/* 格子大小：按视频比例自适应，不超过容器。 */
.video-cell {
  background: #0b0b0b;
  color: #fff;
  border-radius: 6px;
  overflow: hidden;
  position: relative;
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
}
.video-grid--1x1 .video-cell { aspect-ratio: 16 / 9; max-height: 100%; }
.video-grid--2x2 .video-cell { aspect-ratio: 16 / 9; }
.video-grid--3x3 .video-cell { aspect-ratio: 16 / 9; }
.video-grid--4x4 .video-cell { aspect-ratio: 16 / 9; }
.video-cell.is-primary { box-shadow: 0 0 0 2px var(--el-color-danger); }
.video-cell__header { display: flex; gap: 8px; align-items: center; padding: 6px 10px; background: rgba(0,0,0,.6); font-size: var(--text-sm); flex: 0 0 auto; }
.video-cell__no { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; opacity: 0.7; }
.video-cell__title { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.video-cell__body { flex: 1 1 auto; display: flex; align-items: center; justify-content: center; background: #111; position: relative; min-height: 0; }
.video-cell__footer { display: flex; justify-content: space-between; align-items: center; padding: 4px 10px; background: rgba(0,0,0,.6); font-size: var(--text-xs); flex: 0 0 auto; }
.video-element { width: 100%; height: 100%; object-fit: contain; background: #000; }
.video-placeholder { color: #555; text-align: center; }
.placeholder-tip { font-size: var(--text-sm); margin-top: 4px; opacity: 0.6; }

/* 错误提示也走收缩 */
.play-error { padding: 0 16px 8px; flex: 0 0 auto; }

.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
.small { font-size: var(--text-xs); }

/* 窄屏：上下堆叠，左侧栏拿到自然高度，主区域继续 flex 1 */
@media (max-width: 768px) {
  .live-page { height: auto; }
  .live-page :deep(.el-row),
  .live-page :deep(.el-row > .el-col) { display: block; }
  .grid-card { min-height: 540px; }
}
</style>