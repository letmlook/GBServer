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
            <video :src="playUrl" controls autoplay class="video" />
            <el-slider
              v-model="seekPos"
              :max="duration"
              :show-tooltip="false"
              class="seek"
              @change="onSeek"
            />
          </div>
          <el-empty v-else description="从左侧选择录像片段开始回放" />
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
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
    const data = res.data ?? { streamId: '', playUrl: '' }
    currentStreamId.value = data.streamId
    playUrl.value = data.playUrl
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
  ElMessage.success('已停止')
}

onMounted(() => {})
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
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
