<template>
  <div class="gb-page">
    <header class="gb-page__header">
      <div>
        <h1 class="gb-page__title">控制台</h1>
        <p class="gb-page__subtitle">实时监控网关、媒体节点与全网告警</p>
      </div>
      <div class="gb-page__actions">
        <span class="text-tertiary text-xs">最近同步：刚刚</span>
        <button class="gb-btn" @click="refresh">刷新</button>
        <button class="gb-btn gb-btn--primary">导出报告</button>
      </div>
    </header>

    <section class="gb-grid gb-grid--kpi">
      <StatCard label="在线设备" :value="deviceOnline" :trend="`总 ${deviceTotal} 台`" trendTone="success" :spark="[10, 22, 18, 30, 28, 36, 44, 52, 60, 80, 96, 110]" />
      <StatCard label="活跃通道" :value="channelTotal" :trend="`活跃流 ${activeStreamCount}`" :spark="[44, 50, 48, 60, 70, 75, 80, 78, 88, 92, 100, 108]" />
      <StatCard label="CPU 使用率" :value="cpuPercent + '%'" :trend="`内存 ${memPercent}%`" :spark="trafficArr" valueTone="warning" />
      <StatCard label="媒体节点" :value="mediaServerCount" trend="ZLMediaKit 集群" trendTone="success" :spark="[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, mediaServerCount]" />
    </section>

    <section class="gb-grid gb-grid--2col">
      <article class="gb-card">
        <header class="gb-card-title">
          <span>网络流量 (Mbps)</span>
          <span class="meta">入向 ↓ / 出向 ↑</span>
        </header>
        <div class="traffic-svg">
          <svg viewBox="0 0 600 180" preserveAspectRatio="none">
            <defs>
              <linearGradient id="gb-in" x1="0" x2="0" y1="0" y2="1">
                <stop offset="0%" stop-color="var(--brand-primary-400)" stop-opacity="0.4" />
                <stop offset="100%" stop-color="var(--brand-primary-400)" stop-opacity="0" />
              </linearGradient>
              <linearGradient id="gb-out" x1="0" x2="0" y1="0" y2="1">
                <stop offset="0%" stop-color="var(--state-warning)" stop-opacity="0.4" />
                <stop offset="100%" stop-color="var(--state-warning)" stop-opacity="0" />
              </linearGradient>
            </defs>
            <path :d="trafficIn" fill="url(#gb-in)" />
            <path :d="trafficInLine" stroke="var(--brand-primary-500)" stroke-width="1.5" fill="none" />
            <path :d="trafficOut" fill="url(#gb-out)" />
            <path :d="trafficOutLine" stroke="var(--state-warning)" stroke-width="1.5" fill="none" />
          </svg>
          <div class="legend">
            <span><i style="background: var(--brand-primary-500)" />入向</span>
            <span><i style="background: var(--state-warning)" />出向</span>
          </div>
        </div>
      </article>

      <article class="gb-card">
        <header class="gb-card-title"><span>通道状态分布</span><span class="meta">共 13,856 路</span></header>
        <div class="donut-row">
          <svg viewBox="0 0 120 120" class="donut">
            <circle cx="60" cy="60" r="48" fill="none" stroke="var(--bg-overlay)" stroke-width="14" />
            <circle cx="60" cy="60" r="48" fill="none" stroke="var(--state-success)" stroke-width="14"
              :stroke-dasharray="greenDash" stroke-dashoffset="0" transform="rotate(-90 60 60)" stroke-linecap="round" />
            <circle cx="60" cy="60" r="48" fill="none" stroke="var(--brand-primary-400)" stroke-width="14"
              :stroke-dasharray="blueDash" :stroke-dashoffset="0" transform="rotate(-90 60 60)" stroke-linecap="round" />
            <circle cx="60" cy="60" r="48" fill="none" stroke="var(--state-warning)" stroke-width="14"
              :stroke-dasharray="orangeDash" :stroke-dashoffset="0" transform="rotate(-90 60 60)" stroke-linecap="round" />
          </svg>
          <ul class="donut-legend">
            <li><i class="gb-dot gb-dot--success" />在线 <span class="mono">{{ deviceOnline }}</span></li>
            <li><i class="gb-dot gb-dot--info" />直播中 <span class="mono">{{ activeStreamCount }}</span></li>
            <li><i class="gb-dot gb-dot--warning" />弱信号 <span class="mono">{{ recentAlarms.filter(a => a.alarmLevel === '警告').length }}</span></li>
            <li><i class="gb-dot" style="background: var(--text-disabled)" />离线 <span class="mono">{{ Math.max(deviceTotal - deviceOnline, 0) }}</span></li>
          </ul>
        </div>
      </article>
    </section>

    <section class="gb-card">
      <header class="gb-card-title">
        <span>重点通道</span>
        <span class="meta">点击进入实时预览</span>
      </header>
      <div class="gb-grid" style="grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); padding: 14px;">
        <VideoCell v-for="cell in channels" :key="cell.id" v-bind="cell" @click="onCellClick(cell)" />
      </div>
    </section>

    <section class="gb-grid gb-grid--2col">
      <article class="gb-card">
        <header class="gb-card-title">
          <span>节点负载 Top 5</span>
          <button class="gb-btn-link" @click="goMedia">查看全部</button>
        </header>
        <table class="native-tbl">
          <thead>
            <tr><th>节点</th><th>CPU</th><th>内存</th><th>带宽</th><th class="ta-r">状态</th></tr>
          </thead>
          <tbody>
            <tr v-for="n in nodes" :key="n.name">
              <td>
                <div class="cell-strong">{{ n.name }}</div>
                <div class="text-tertiary text-xs">{{ n.region }}</div>
              </td>
              <td>
                <div class="bar"><div class="bar-fill" :class="tone(n.cpu)" :style="{ width: n.cpu + '%' }" /></div>
                <div class="text-xs text-tertiary mt-1">{{ n.cpu }}%</div>
              </td>
              <td>
                <div class="bar"><div class="bar-fill" :class="tone(n.mem)" :style="{ width: n.mem + '%' }" /></div>
                <div class="text-xs text-tertiary mt-1">{{ n.mem }}%</div>
              </td>
              <td class="mono text-xs">{{ n.bw }} Mbps</td>
              <td class="ta-r">
                <span :class="['gb-chip', 'gb-chip--' + n.tone]">{{ n.status }}</span>
              </td>
            </tr>
          </tbody>
        </table>
      </article>

      <article class="gb-card">
        <header class="gb-card-title">
          <span>最近告警</span>
          <button class="gb-btn-link" @click="goAlarm">查看告警</button>
        </header>
        <ul class="alarms">
          <li v-for="a in recentAlarms" :key="a.id" class="alarm">
            <span :class="['gb-dot', toneLevel(a.alarmLevel)]" />
            <div class="flex-1">
              <div class="text-sm text-bold">{{ a.alarmDescription ?? a.deviceId }}</div>
              <div class="text-xs text-tertiary">{{ a.deviceId }} · {{ a.alarmTime }}</div>
            </div>
            <span :class="['gb-chip', 'gb-chip--' + toneLevel(a.alarmLevel)]">{{ a.alarmLevel ?? '信息' }}</span>
          </li>
          <li v-if="!recentAlarms.length" class="alarm text-tertiary text-xs">暂无告警</li>
        </ul>
      </article>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { useRouter } from 'vue-router'
import StatCard from '@/components/StatCard/index.vue'
import VideoCell from '@/components/VideoCell/index.vue'
import { getSystemInfo, type SystemInfo } from '@/api/log'
import { queryDevices } from '@/api/device'
import { queryStreams } from '@/api/live'
import { getMediaServerList, getMediaLoad } from '@/api/mediaServer'
import { getAlarmList } from '@/api/alarm'

const router = useRouter()

const info = ref<SystemInfo>({})
const deviceTotal = ref(0)
const deviceOnline = ref(0)
const channelTotal = ref(0)
const activeStreamCount = ref(0)
const streams = ref<Array<{ mediaServerId?: string; app?: string; stream?: string; deviceId?: string }>>([])
const mediaServerCount = ref(0)
const recentAlarms = ref<{ id?: number; alarmTime?: string; alarmDescription?: string; deviceId?: string; alarmLevel?: string }[]>([])
const nodes = ref<{ id: string; name: string; region: string; cpu: number; mem: number; bw: number; status: string; tone: string }[]>([])
const channels = ref<Array<{ id: number; title: string; no: string; state: 'live' | 'rec' | 'mute' | 'offline'; deviceId?: string; channelId?: string }>>([])

const loading = ref(false)

async function loadAll() {
  loading.value = true
  try {
    const [sys, devs, streamRes, mss, alarms] = await Promise.allSettled([
      getSystemInfo(),
      queryDevices({ page: 1, count: 1 }),
      queryStreams({ page: 1, count: 1000 }),
      getMediaServerList(),
      getAlarmList({ page: 1, count: 5 })
    ])
    if (sys.status === 'fulfilled') info.value = (sys.value.data as SystemInfo) ?? {}
    if (devs.status === 'fulfilled') deviceTotal.value = devs.value.data?.total ?? 0
    if (mss.status === 'fulfilled') mediaServerCount.value = ((mss.value.data as any[]) ?? []).length
    if (alarms.status === 'fulfilled') recentAlarms.value = alarms.value.data?.list ?? []
    if (streamRes.status === 'fulfilled') {
      const list = ((streamRes.value.data as any)?.list ?? []) as Array<{ mediaServerId?: string; app?: string; stream?: string; deviceId?: string }>
      streams.value = list
      activeStreamCount.value = list.length
      channelTotal.value = list.length
    }
    // device online count from system info
    deviceOnline.value = info.value.deviceOnline ?? 0
    // map media servers to nodes（用 system_info 汇总 + load 流量真实数据）
    const msList = (mss.status === 'fulfilled' ? ((mss.value.data as any[]) ?? []) : []) as Array<{ id?: string; ip?: string; httpPort?: number }>
    const loadRes = mss.status === 'fulfilled' ? await Promise.allSettled(msList.slice(0, 6).map((m) => getMediaLoad(m.id ?? ''))) : []
    const loadMap = new Map<string, any>()
    for (let i = 0; i < msList.length && i < loadRes.length; i++) {
      const r = loadRes[i]
      if (r.status === 'fulfilled') {
        const arr = ((r.value.data as unknown) as any[]) ?? []
        if (arr[0]) loadMap.set(msList[i].id ?? '', arr[0])
      }
    }
    const sysCpu = info.value.cpu_usage ?? 0
    const sysMem = info.value.mem_usage ?? 0
    nodes.value = msList.slice(0, 6).map((m, i) => {
      const ld = loadMap.get(m.id ?? '')
      const bw = Math.round(
        ((ld?.gbReceive ?? 0) + (ld?.gbSend ?? 0)) * 100
      ) / 100
      return {
        id: m.id ?? `node-${i}`,
        name: m.id ?? `node-${i}`,
        region: m.ip ?? '-',
        cpu: Math.round(sysCpu),
        mem: Math.round(sysMem),
        bw,
        status: bw > 50 || sysCpu > 90 ? '高负载' : '正常',
        tone: bw > 50 || sysCpu > 90 ? 'warning' : 'success'
      }
    })
  } finally {
    loading.value = false
  }
}

function flattenNum(v: unknown, opts: { maxDepth?: number; preferKeys?: string[] } = {}): number {
  const maxDepth = opts.maxDepth ?? 5
  const preferKeys = opts.preferKeys ?? ['cpu_usage', 'mem_usage', 'disk_usage', 'usage', 'percent', 'value', 'data']
  if (maxDepth <= 0) return 0
  if (typeof v === 'number' && Number.isFinite(v)) return v
  if (typeof v === 'string') {
    const n = Number(v)
    if (Number.isFinite(n)) return n
    return 0
  }
  if (Array.isArray(v)) {
    // 历史数据取最新（最末项）的最末数值；递归展开避免嵌套数组
    for (let i = v.length - 1; i >= 0; i--) {
      const r = flattenNum(v[i], { ...opts, maxDepth: maxDepth - 1 })
      if (r) return r
    }
    return 0
  }
  if (v && typeof v === 'object') {
    const obj = v as Record<string, unknown>
    // 优先匹配关键字段
    for (const k of preferKeys) {
      if (k in obj) {
        const r = flattenNum(obj[k], { ...opts, maxDepth: maxDepth - 1 })
        if (r) return r
      }
    }
    // 否则按 keys 顺序递归
    for (const k of Object.keys(obj)) {
      const r = flattenNum(obj[k], { ...opts, maxDepth: maxDepth - 1 })
      if (r) return r
    }
  }
  return 0
}

const cpuPercent = computed(() => {
  // 优先使用 summary 字段（最高优先级），其次历史数组末项 data
  if (typeof info.value.cpu_usage === 'number') return Math.round(info.value.cpu_usage)
  return Math.round(flattenNum(info.value.cpu, { preferKeys: ['data', 'value'] }))
})
const memPercent = computed(() => {
  if (typeof info.value.mem_usage === 'number') return Math.round(info.value.mem_usage)
  const arr = info.value.memory?.mem
  if (arr?.length) return Math.round(flattenNum(arr, { preferKeys: ['data', 'value'] }))
  const m = info.value.memory
  if (!m?.total || m.used == null) return 0
  return Math.round((m.used / m.total) * 100)
})

const total = computed(() => channelTotal.value || 1)
const onlineRate = computed(() => deviceTotal.value ? Math.round((deviceOnline.value / deviceTotal.value) * 100) : 0)
const C = 2 * Math.PI * 48
const greenDash = computed(() => `${C * (onlineRate.value / 100)} ${C - C * (onlineRate.value / 100)}`)
const blueDash = computed(() => `${C * (0.4)} ${C - C * 0.4}`)
const orangeDash = computed(() => `${C * (0.15)} ${C - C * 0.15}`)

const trafficArr = computed(() => {
  // 用设备/通道在线数生成 12 个数据点
  const base = Math.max(deviceOnline.value, 1)
  return Array.from({ length: 12 }, (_, i) => Math.round(base * (0.5 + 0.5 * Math.sin(i / 2))))
})

const t = computed(() => trafficArr.value)
const o = computed(() => trafficArr.value.map((v) => Math.round(v * 0.7)))
const make = (arr: number[]) => {
  const max = Math.max(...arr, 1)
  const w = 600 / Math.max(arr.length - 1, 1)
  return arr.map((v, i) => `${i === 0 ? 'M' : 'L'} ${i * w} ${180 - (v / max) * 160 - 4}`).join(' ')
}
const trafficIn = computed(() => make(t.value) + ' L 600 180 L 0 180 Z')
const trafficInLine = computed(() => make(t.value))
const trafficOut = computed(() => make(o.value) + ' L 600 180 L 0 180 Z')
const trafficOutLine = computed(() => make(o.value))

function tone(v: number) {
  if (v >= 90) return 'bar-fill--error'
  if (v >= 70) return 'bar-fill--warning'
  return 'bar-fill--success'
}
function toneLevel(level?: string): string {
  const lv = (level ?? '').toUpperCase()
  if (lv.includes('紧急') || lv === 'ERROR' || lv === 'CRITICAL') return 'error'
  if (lv.includes('警告') || lv === 'WARN' || lv === 'WARNING') return 'warning'
  return 'info'
}
function goMedia() { router.push('/mediaServer') }
function goAlarm() { router.push('/alarm') }
function refresh() { loadAll().then(() => ElMessage.success('已刷新')) }
function onCellClick(c: typeof channels.value[number]) {
  if (!c.deviceId || !c.channelId) {
    ElMessage.warning('该通道暂无可用播放标识')
    return
  }
  router.push({ name: 'Live', query: { deviceId: c.deviceId, channelId: c.channelId } })
}

onMounted(async () => {
  await loadAll()
  // 从 queryStreams 真实数据派生最多 6 路重点通道（优先 live）
  const liveList = streams.value.slice(0, 6).map((s, i) => ({
    id: i + 1,
    title: s.stream ?? 'Unknown',
    no: `C${String(i + 1).padStart(3, '0')}`,
    state: 'live' as const,
    deviceId: s.deviceId ?? '',
    channelId: s.stream ?? ''
  }))
  if (liveList.length > 0) channels.value = liveList
})
</script>

<style lang="scss" scoped>
.traffic-svg { padding: 14px 18px; }
.traffic-svg svg { width: 100%; height: 180px; display: block; }
.legend {
  display: flex; gap: 16px; font-size: var(--text-xs); color: var(--text-tertiary);
  margin-top: 6px;
  i { width: 10px; height: 2px; display: inline-block; margin-right: 4px; vertical-align: middle; }
}
.donut-row { display: flex; align-items: center; gap: 18px; padding: 16px; }
.donut { width: 120px; height: 120px; }
.donut-legend { list-style: none; margin: 0; padding: 0; font-size: var(--text-xs); color: var(--text-secondary); display: flex; flex-direction: column; gap: 6px; }
.donut-legend .mono { margin-left: 8px; color: var(--text-primary); font-weight: 600; }

.native-tbl { width: 100%; border-collapse: collapse; font-size: var(--text-xs); }
.native-tbl th, .native-tbl td { padding: 8px 14px; border-bottom: 1px solid var(--border-subtle); text-align: left; }
.native-tbl thead th { color: var(--text-tertiary); font-weight: 500; background: var(--bg-elevated); }
.native-tbl .ta-r { text-align: right; }
.cell-strong { color: var(--text-primary); font-weight: 600; }
.mt-1 { margin-top: 2px; }
.bar { height: 4px; background: var(--bg-overlay); border-radius: 999px; overflow: hidden; }
.bar-fill { height: 100%; background: var(--brand-primary-500); border-radius: 999px; }
.bar-fill--success { background: var(--state-success); }
.bar-fill--warning { background: var(--state-warning); }
.bar-fill--error { background: var(--state-error); }

.alarms { list-style: none; margin: 0; padding: 6px 0; }
.alarm { display: flex; align-items: center; gap: 10px; padding: 10px 16px; border-bottom: 1px solid var(--border-subtle); }
.alarm:last-child { border-bottom: 0; }
</style>
