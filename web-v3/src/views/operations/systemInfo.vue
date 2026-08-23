<template>
  <div class="system-info-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">系统信息</h1>
        <p class="page-subtitle">{{ info.version ?? '加载中...' }} · 运行时长 {{ uptimeText }}</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
      </div>
    </div>

    <el-row :gutter="12" v-loading="loading">
      <el-col :xs="24" :md="6"><el-card class="metric-card">
        <div class="metric-label">CPU</div>
        <div class="metric-value">{{ info.cpu ?? 0 }}%</div>
        <el-progress :percentage="info.cpu ?? 0" :stroke-width="6" :show-text="false" :color="cpuColor(info.cpu ?? 0)" />
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
          <el-table :data="resourceRows" stripe>
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
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getSystemInfo, type SystemInfo } from '@/api/log'

const loading = ref(false)
const info = ref<SystemInfo>({})

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
}

onMounted(loadData)
</script>

<style scoped>
.system-info-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.metric-card { text-align: center; }
.metric-label { color: var(--el-text-color-secondary); font-size: 12px; }
.metric-value { font-size: 28px; font-weight: 700; margin: 6px 0; }
.metric-detail { font-size: 11px; color: var(--el-text-color-secondary); margin-top: 6px; }
.mono { font-family: ui-monospace, monospace; font-size: 13px; }
</style>
