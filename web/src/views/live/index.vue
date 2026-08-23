<template>
  <div class="live-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">实时直播</h1>
        <p class="page-subtitle">{{ stats.online }} 路在线 · {{ stats.total }} 路总计</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
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
                <el-tag v-if="data.status" :type="data.status === 'ON' ? 'success' : 'info'" size="small">{{ data.status === 'ON' ? 'ON' : 'OFF' }}</el-tag>
              </span>
            </template>
          </el-tree>
        </el-card>
      </el-col>

      <el-col :xs="24" :md="18">
        <el-card class="grid-card" v-loading="loading">
          <div v-if="!currentChannel" class="empty">
            <el-empty description="请从左侧选择通道开始播放" />
          </div>
          <div v-else>
            <div :class="['video-grid', `video-grid--${layout}`]">
              <div v-for="(cell, idx) in cells" :key="idx" class="video-cell" :class="{ 'is-primary': cell.primary }">
                <div class="video-cell__header">
                  <span class="video-cell__no">{{ String(idx + 1).padStart(2, '0') }}</span>
                  <span class="video-cell__title">{{ cell.name }}</span>
                  <el-tag v-if="cell.primary" type="danger" size="small" effect="dark">● LIVE</el-tag>
                </div>
                <div class="video-cell__body">
                  <video v-if="cell.primary && cell.url" :src="cell.url" controls autoplay muted class="video-element" />
                  <div v-else class="video-placeholder">
                    <el-icon size="32"><VideoCameraFilled /></el-icon>
                  </div>
                </div>
                <div class="video-cell__footer">
                  <span class="mono small">{{ cell.id ?? '-' }}</span>
                  <el-button-group size="small">
                    <el-button @click="onSnap(cell)">抓图</el-button>
                    <el-button @click="onStop(cell)" type="danger" plain>停止</el-button>
                  </el-button-group>
                </div>
              </div>
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
import { computed, nextTick, onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import {
  ArrowUp,
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  VideoPause,
  VideoCameraFilled
} from '@element-plus/icons-vue'
import {
  startPlay,
  stopPlay,
  playSnap,
  getPlayUrl,
  sendPtz as sendPtzApi,
  queryStreams
} from '@/api/live'
import { queryDeviceTree } from '@/api/device'

const route = useRoute()
const router = useRouter()
const loading = ref(false)
const kw = ref('')
const layout = ref<'2x2' | '3x3' | '4x4'>('2x2')
const tree = ref<any[]>([])
const treeRef = ref<any>()
const streams = ref<{ deviceId: string; channelId: string; app: string; stream: string }[]>([])
const currentChannel = ref<{ deviceId: string; channelId: string; name?: string } | null>(null)
const cells = ref<any[]>([])

const stats = computed(() => ({
  total: streams.value.length,
  online: streams.value.length
}))

watch(kw, (v) => treeRef.value?.filter(v))

function filterNode(value: string, data: any) {
  if (!value) return true
  return data.name?.includes(value) ?? false
}

async function loadData() {
  loading.value = true
  try {
    const res = await queryStreams({ page: 1, count: 1000 })
    const list = res.data?.list ?? []
    streams.value = list.map((s: any) => ({
      deviceId: s.mediaServerId ?? '',
      channelId: s.stream ?? '',
      app: s.app ?? '',
      stream: s.stream ?? ''
    }))

    // 构建设备树（按 deviceId 分组）
    const grouped = new Map<string, any[]>()
    for (const s of streams.value) {
      const arr = grouped.get(s.deviceId) ?? []
      arr.push({ id: `${s.deviceId}:${s.channelId}`, name: s.stream, raw: s })
      grouped.set(s.deviceId, arr)
    }
    tree.value = Array.from(grouped.entries()).map(([deviceId, children]) => ({
      id: deviceId,
      name: deviceId,
      children
    }))
  } finally {
    loading.value = false
  }
}

async function onNodeClick(node: any) {
  if (!node.raw) return
  await playChannel(node.raw)
}

async function playChannel(s: { deviceId: string; channelId: string }) {
  try {
    const res = await startPlay(s.deviceId, s.channelId)
    const data = res.data ?? { streamId: '', playUrl: '' }
    const url = data.playUrl || (await getPlayUrl({ deviceId: s.deviceId, channelId: s.channelId })).data?.url || ''
    currentChannel.value = { ...s, name: data.streamId || s.channelId }
    buildGrid(s, url)
  } catch (e: any) {
    ElMessage.error(e?.message ?? '播放失败')
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
  if (!cell?.deviceId || !cell?.channelId) return
  await stopPlay(cell.deviceId, cell.channelId)
  ElMessage.success('已停止')
  buildGrid(cell, '')
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

onMounted(async () => {
  await loadData()
  const qd = route.query.deviceId as string | undefined
  const qc = route.query.channelId as string | undefined
  if (qd && qc) {
    await nextTick()
    await playChannel({ deviceId: qd, channelId: qc })
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
.video-grid { display: grid; gap: 6px; }
.video-grid--2x2 { grid-template-columns: repeat(2, 1fr); }
.video-grid--3x3 { grid-template-columns: repeat(3, 1fr); }
.video-grid--4x4 { grid-template-columns: repeat(4, 1fr); }
.video-cell { background: #0b0b0b; color: #fff; border-radius: 6px; overflow: hidden; aspect-ratio: 16/9; position: relative; display: flex; flex-direction: column; }
.video-cell.is-primary { box-shadow: 0 0 0 2px var(--el-color-danger); }
.video-cell__header { display: flex; gap: 8px; align-items: center; padding: 6px 10px; background: rgba(0,0,0,.6); font-size: 12px; }
.video-cell__no { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; opacity: 0.7; }
.video-cell__title { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.video-cell__body { flex: 1; display: flex; align-items: center; justify-content: center; background: #111; }
.video-cell__footer { display: flex; justify-content: space-between; align-items: center; padding: 4px 10px; background: rgba(0,0,0,.6); font-size: 11px; }
.video-element { width: 100%; height: 100%; object-fit: contain; background: #000; }
.video-placeholder { color: #555; }
.ptz-bar { padding: 12px; display: flex; align-items: center; }
.ptz-title { margin-right: 8px; color: var(--el-text-color-secondary); }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
.small { font-size: 11px; }
</style>
