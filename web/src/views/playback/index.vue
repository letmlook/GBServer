<template>
  <div class="playback-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">录像回放</h1>
        <p class="page-subtitle">GB/T 28181 录像检索 · 回放控制</p>
      </div>
    </div>

    <el-card class="filter-card">
      <el-form :inline="true">
        <el-form-item label="设备">
          <el-input v-model="form.deviceId" placeholder="国标设备ID" />
        </el-form-item>
        <el-form-item label="通道">
          <el-input v-model="form.channelId" placeholder="国标通道ID" />
        </el-form-item>
        <el-form-item label="开始">
          <el-date-picker v-model="form.startTime" type="datetime" placeholder="开始时间" />
        </el-form-item>
        <el-form-item label="结束">
          <el-date-picker v-model="form.endTime" type="datetime" placeholder="结束时间" />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="onQuery">检索</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <el-row :gutter="12">
      <el-col :xs="24" :md="10">
        <el-card class="result-card" v-loading="loading">
          <template #header>
            <span>录像列表 · 共 {{ records.length }} 条</span>
          </template>
          <el-table :data="records" height="500" highlight-current-row @row-click="onSelect">
            <el-table-column prop="startTime" label="开始" min-width="160">
              <template #default="{ row }">
                <span class="mono">{{ row.startTime }}</span>
              </template>
            </el-table-column>
            <el-table-column prop="endTime" label="结束" min-width="160">
              <template #default="{ row }">
                <span class="mono">{{ row.endTime }}</span>
              </template>
            </el-table-column>
            <el-table-column prop="name" label="名称" min-width="120" show-overflow-tooltip />
          </el-table>
        </el-card>
      </el-col>

      <el-col :xs="24" :md="14">
        <el-card class="player-card" v-loading="playerLoading">
          <template #header>
            <div class="player-header">
              <span>{{ currentRecord ? currentRecord.name : '请选择录像片段' }}</span>
              <div v-if="currentStreamId" class="player-controls">
                <el-button-group size="small">
                  <el-button @click="control('pause')">暂停</el-button>
                  <el-button @click="control('resume')">继续</el-button>
                  <el-button @click="control('speed', 0.5)">0.5×</el-button>
                  <el-button @click="control('speed', 1)">1×</el-button>
                  <el-button @click="control('speed', 2)">2×</el-button>
                  <el-button @click="control('speed', 4)">4×</el-button>
                </el-button-group>
                <el-button size="small" type="danger" plain @click="onStop">停止</el-button>
              </div>
            </div>
          </template>

          <div v-if="playUrl" class="player-body">
            <!-- 浏览器不能直接播 RTSP：后端同时给了 hls/flv，这里按同一约定播放 -->
            <video ref="videoRef" controls autoplay class="video" />
            <el-slider
              v-model="seekPos"
              :max="duration"
              :show-tooltip="false"
              class="seek"
              @change="onSeek"
            />
            <div class="play-url mono" :title="playUrl">{{ playUrl }}</div>
          </div>
          <el-empty v-else description="从左侧选择录像片段开始回放" />
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import {
  startPlayback,
  stopPlayback,
  pausePlayback,
  resumePlayback,
  seekPlayback,
  speedPlayback,
  queryGbRecord
} from '@/api/playback'

const loading = ref(false)
const playerLoading = ref(false)
const records = ref<any[]>([])
const currentRecord = ref<any>(null)
const currentStreamId = ref('')
const playUrl = ref('')
const seekPos = ref(0)
const duration = ref(3600)
const videoRef = ref<HTMLVideoElement>()

// 与实时预览页同一套播放约定：HLS(hls.js) → FLV(flv.js) → 原生
let hlsInstance: any = null
let flvPlayer: any = null
let playIndex = 0
const PLAY_SEQ = ++playIndex

function destroyPlayers() {
  try { hlsInstance?.destroy?.() } catch {}
  hlsInstance = null
  try { flvPlayer?.destroy?.() } catch {}
  flvPlayer = null
  const v = videoRef.value
  if (v) {
    v.removeAttribute('src')
    v.load()
  }
}

async function attachVideo(url: string) {
  await nextTick()
  const video = videoRef.value
  if (!video || !url) return
  destroyPlayers()
  const seq = PLAY_SEQ

  // Safari 原生 HLS
  if (video.canPlayType('application/vnd.apple.mpegurl') !== '' && url.includes('.m3u8')) {
    video.src = url
    return
  }
  if (url.includes('.m3u8')) {
    try {
      const Hls = (await import('hls.js')).default
      if (seq !== PLAY_SEQ) return
      if (Hls.isSupported()) {
        const hls = new Hls({ enableWorker: true })
        hls.loadSource(url)
        hls.attachMedia(video)
        hls.on(Hls.Events.ERROR, (_e: any, data: any) => {
          if (data?.fatal) ElMessage.error(`HLS 播放失败: ${data?.details ?? data?.type ?? 'unknown'}`)
        })
        hlsInstance = hls
        return
      }
    } catch (e: any) {
      ElMessage.warning(`hls.js 不可用: ${e?.message ?? e}`)
    }
  }
  if (url.includes('.flv')) {
    try {
      const flvjs = (await import('flv.js')).default
      if (seq !== PLAY_SEQ) return
      if (flvjs.isSupported()) {
        const player = flvjs.createPlayer({ type: 'flv', url, isLive: false })
        player.attachMediaElement(video)
        player.load()
        player.play()
        flvPlayer = player
        return
      }
    } catch (e: any) {
      ElMessage.warning(`flv.js 不可用: ${e?.message ?? e}`)
    }
  }
  // 最后兜底：交给浏览器（mp4 等）
  video.src = url
}

const form = reactive({
  deviceId: '',
  channelId: '',
  startTime: undefined as Date | undefined,
  endTime: undefined as Date | undefined
})

async function onQuery() {
  if (!form.deviceId || !form.channelId) {
    ElMessage.warning('请填写设备ID和通道ID')
    return
  }
  loading.value = true
  try {
    const res = await queryGbRecord({
      deviceId: form.deviceId,
      channelId: form.channelId,
      startTime: form.startTime?.toISOString(),
      endTime: form.endTime?.toISOString()
    })
    records.value = res.data?.list ?? []
  } catch {
    records.value = []
  } finally {
    loading.value = false
  }
}

async function onSelect(row: any) {
  currentRecord.value = row
  playerLoading.value = true
  try {
    const res = await startPlayback(form.deviceId, form.channelId, {
      startTime: row.startTime,
      endTime: row.endTime
    })
    const data = res.data
    if (!data?.streamId) {
      throw new Error(res.msg || '后端未返回回放流')
    }
    // 优先 HLS（浏览器兼容性最好），其次 FLV；RTSP 仅作最后兜底
    const url = data.hls || data.flvUrl || data.playUrl || ''
    if (!url) {
      throw new Error('后端未返回可用的播放地址')
    }
    currentStreamId.value = data.streamId
    playUrl.value = url
    await attachVideo(url)
  } catch (e: any) {
    ElMessage.error(e?.message ?? '回放启动失败')
  } finally {
    playerLoading.value = false
  }
}

async function control(action: 'pause' | 'resume' | 'speed', speed?: number) {
  if (!currentStreamId.value) return
  try {
    if (action === 'pause') await pausePlayback(currentStreamId.value)
    if (action === 'resume') await resumePlayback(currentStreamId.value)
    if (action === 'speed' && speed) await speedPlayback(currentStreamId.value, speed)
    ElMessage.success('已发送')
  } catch (e: any) {
    ElMessage.error(e?.message ?? '操作失败')
  }
}

async function onSeek(value: number | number[]) {
  const v = Array.isArray(value) ? value[0] : value
  if (!currentStreamId.value) return
  await seekPlayback(currentStreamId.value, v)
}

async function onStop() {
  if (!currentStreamId.value) return
  await stopPlayback(form.deviceId, form.channelId, currentStreamId.value)
  currentStreamId.value = ''
  playUrl.value = ''
  currentRecord.value = null
  destroyPlayers()
  ElMessage.success('已停止')
}

onMounted(() => {})
onUnmounted(destroyPlayers)
</script>

<style scoped>
.playback-page { padding: 16px; }
.page-header { margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.filter-card { margin-bottom: 12px; }
.result-card { min-height: 540px; }
.player-card { min-height: 540px; }
.player-header { display: flex; justify-content: space-between; align-items: center; }
.player-controls { display: flex; gap: 8px; }
.player-body { padding: 12px; }
.video { width: 100%; aspect-ratio: 16/9; background: #000; border-radius: 6px; }
.seek { margin-top: 8px; }
.play-url { margin-top: 6px; color: var(--el-text-color-secondary); font-size: 12px; word-break: break-all; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
