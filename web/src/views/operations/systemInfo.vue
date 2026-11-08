<template>
  <div class="system-info-page">
    <div class="page-header">
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
      </div>
    </div>

    <el-row :gutter="12" v-loading="loading">
      <el-col :xs="24" :md="6"><el-card class="metric-card">
        <div class="metric-label">CPU</div>
        <div class="metric-value">{{ cpuPercent }}%</div>
        <el-progress :percentage="cpuPercent" :stroke-width="6" :show-text="false" :color="cpuColor(cpuPercent)" />
      </el-card></el-col>

      <el-col :xs="24" :md="6"><el-card class="metric-card">
        <div class="metric-label">内存</div>
        <div class="metric-value">{{ memPercent }}%</div>
        <el-progress :percentage="memPercent" :stroke-width="6" :show-text="false" :color="cpuColor(memPercent)" />
        <div class="metric-detail">{{ formatSize(info.memory?.used) }} / {{ formatSize(info.memory?.total) }}</div>
      </el-card></el-col>

      <el-col :xs="24" :md="6"><el-card class="metric-card">
        <div class="metric-label">磁盘</div>
        <div class="metric-value">{{ diskPercent }}%</div>
        <el-progress :percentage="diskPercent" :stroke-width="6" :show-text="false" :color="cpuColor(diskPercent)" />
        <div class="metric-detail">{{ formatSize(info.disk?.[0]?.used) }} / {{ formatSize(info.disk?.[0]?.total) }}</div>
      </el-card></el-col>

      <el-col :xs="24" :md="6"><el-card class="metric-card">
        <div class="metric-label">运行时长</div>
        <div class="metric-value mono">{{ uptimeText }}</div>
        <div class="metric-detail">服务启动时间：{{ startTimeText }}</div>
      </el-card></el-col>
    </el-row>

    <el-row :gutter="12" style="margin-top: 12px">
      <el-col :xs="24" :md="12">
        <el-card>
          <template #header><span>资源统计</span></template>
          <el-table table-layout="auto" :data="resourceRows" stripe>
            <el-table-column prop="key" label="资源" min-width="160" />
            <el-table-column prop="value" label="数量" width="160">
              <template #default="{ row }"><span class="mono">{{ row.value }}</span></template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-col>
      <el-col :xs="24" :md="12">
        <el-card>
          <template #header><span>构建信息</span></template>
          <el-descriptions :column="1">
            <el-descriptions-item label="版本">{{ info.version ?? '-' }}</el-descriptions-item>
            <el-descriptions-item label="构建时间">{{ info.buildTime ?? '-' }}</el-descriptions-item>
            <el-descriptions-item label="运行时长">{{ uptimeText }}</el-descriptions-item>
            <el-descriptions-item label="服务">{{ info.mediaServerCount ?? 0 }} 媒体节点</el-descriptions-item>
          </el-descriptions>
        </el-card>
      </el-col>
    </el-row>

    <el-card v-if="isAdmin" class="recovery-card" data-testid="playback-recovery">
      <template #header><span>待清理的回放会话</span></template>
      <p class="recovery-note">以下回放会话的清理尚未完成。重试完成后，设备是否执行仍需确认；记录仅保留在当前服务进程中。</p>
      <el-alert v-if="recoveryError" :title="recoveryError" type="error" :closable="false" show-icon />
      <el-table table-layout="auto" :data="recoveryRows" v-loading="recoveryLoading" stripe empty-text="暂无待清理会话">
        <el-table-column prop="deviceId" label="设备 ID" min-width="170" show-overflow-tooltip />
        <el-table-column prop="channelId" label="通道 ID" min-width="170" show-overflow-tooltip />
        <el-table-column prop="streamId" label="流 ID" min-width="190" show-overflow-tooltip />
        <el-table-column label="设备结果" width="120"><template #default>未确认</template></el-table-column>
        <el-table-column label="操作" width="120" fixed="right">
          <template #default="{ row }"><el-button link type="primary" :loading="retrying === row.streamId" :disabled="retrying !== ''" @click="retryRecovery(row.streamId)">重试清理</el-button></template>
        </el-table-column>
      </el-table>
      <el-pagination v-if="recoveryTotal > recoveryPageSize" class="recovery-pagination" layout="prev, pager, next" :total="recoveryTotal" :page-size="recoveryPageSize" v-model:current-page="recoveryPage" @current-change="loadRecovery" />
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getSystemInfo, type SystemInfo } from '@/api/log'
import { getUserInfo } from '@/api/user'
import { listPlaybackRecovery, retryPlaybackRecovery, type PlaybackRecoveryItem } from '@/api/playbackRecovery'
import { ElMessage } from 'element-plus'

const loading = ref(false)
const info = ref<SystemInfo>({})
const isAdmin = ref(false)
const recoveryRows = ref<PlaybackRecoveryItem[]>([])
const recoveryTotal = ref(0)
const recoveryPage = ref(1)
const recoveryPageSize = 20
const recoveryLoading = ref(false)
const recoveryError = ref('')
const retrying = ref('')

// `info.cpu` 是折线图的历史采样数组；当前使用率在 `cpu_usage`
const cpuPercent = computed(() => Math.round(info.value.cpu_usage ?? 0))

const memPercent = computed(() => {
  const m = info.value.memory
  if (!m?.total || m.used == null) return 0
  return Math.round((m.used / m.total) * 100)
})

const diskPercent = computed(() => {
  const d = info.value.disk?.[0]
  if (!d?.total) return 0
  return Math.round((d.used / d.total) * 100)
})

const uptimeText = computed(() => {
  const s = info.value.uptime ?? 0
  const days = Math.floor(s / 86400)
  const hours = Math.floor((s % 86400) / 3600)
  const mins = Math.floor((s % 3600) / 60)
  return `${days}d ${hours}h ${mins}m`
})

const startTimeText = computed(() => {
  const s = info.value.uptime ?? 0
  if (!s) return '-'
  const t = new Date(Date.now() - s * 1000).toISOString().replace('T', ' ').slice(0, 19)
  return t
})

const resourceRows = computed(() => [
  { key: '媒体节点', value: info.value.mediaServerCount ?? 0 },
  { key: '设备总数 / 在线', value: `${info.value.deviceTotal ?? 0} / ${info.value.deviceOnline ?? 0}` },
  { key: '通道总数 / 在线', value: `${info.value.channelTotal ?? 0} / ${info.value.channelOnline ?? 0}` }
])

function cpuColor(p: number): string {
  if (p < 60) return '#16a34a'
  if (p < 85) return '#f59e0b'
  return '#ef4444'
}

function formatSize(byte?: number): string {
  if (!byte) return '-'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let v = byte
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(1)} ${units[i]}`
}

async function loadData() {
  loading.value = true
  try {
    const res = await getSystemInfo()
    info.value = (res.data as SystemInfo) ?? {}
  } catch {
    info.value = {}
  } finally {
    loading.value = false
  }
  if (isAdmin.value) await loadRecovery()
}

async function loadRecovery() {
  recoveryLoading.value = true
  recoveryError.value = ''
  try {
    const res = await listPlaybackRecovery((recoveryPage.value - 1) * recoveryPageSize, recoveryPageSize)
    recoveryRows.value = res.data?.items ?? []
    recoveryTotal.value = res.data?.total ?? 0
    if (recoveryRows.value.length === 0 && recoveryPage.value > 1 && recoveryTotal.value > 0) {
      recoveryPage.value = Math.ceil(recoveryTotal.value / recoveryPageSize)
      await loadRecovery()
    }
  } catch {
    recoveryError.value = '待清理会话加载失败，请重试'
  } finally {
    recoveryLoading.value = false
  }
}

async function retryRecovery(streamId: string) {
  retrying.value = streamId
  try {
    await retryPlaybackRecovery(streamId)
    ElMessage.success('本地清理已完成，设备执行结果未确认')
    await loadRecovery()
  } catch {
    // The request interceptor reports the failure; keep the row for retry.
  } finally {
    retrying.value = ''
  }
}

onMounted(async () => {
  try {
    const res = await getUserInfo()
    isAdmin.value = res.data?.role?.id === 1 || res.data?.role?.authority?.trim() === '0'
  } catch {
    isAdmin.value = false
  }
  await loadData()
})
</script>

<style scoped>
.system-info-page { padding: 16px; }
.page-header { display: flex; justify-content: flex-end; align-items: flex-end; margin-bottom: 12px; }
.metric-card { text-align: center; }
.metric-label { color: var(--el-text-color-secondary); font-size: var(--text-sm); }
.metric-value { font-size: 28px; font-weight: 700; margin: 6px 0; }
.metric-detail { font-size: var(--text-xs); color: var(--el-text-color-secondary); margin-top: 6px; }
.mono { font-family: ui-monospace, monospace; font-size: var(--text-sm); }
.recovery-card { margin-top: 12px; }
.recovery-note { margin: 0 0 12px; color: var(--el-text-color-secondary); font-size: var(--text-sm); }
.recovery-pagination { justify-content: flex-end; margin-top: 12px; }
</style>
