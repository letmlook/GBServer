<template>
  <div class="gb-page">
    <header class="gb-page__header">
      <div>
        <h1 class="gb-page__title">控制台</h1>
        <p class="gb-page__subtitle">实时监控网关、媒体节点与全网告警</p>
      </div>
      <div class="gb-page__actions">
        <span class="text-tertiary text-xs">最近同步：{{ syncLabel }}（{{ REFRESH_INTERVAL_MS / 1000 }}s 自动）</span>
        <button class="gb-btn" @click="refresh">刷新</button>
        <button class="gb-btn gb-btn--primary">导出报告</button>
      </div>
    </header>

    <section class="gb-grid gb-grid--kpi">
      <StatCard label="在线设备" :value="deviceOnline" :trend="`总 ${deviceTotal} 台`" trendTone="success" />
      <StatCard label="活跃通道" :value="channelTotal" :trend="`在线 ${channelOnline ?? 0} / 直播 ${activeStreamCount}`" />
      <StatCard label="媒体节点" :value="mediaServerCount" :trend="`ZLMediaKit 集群`" trendTone="success" />
      <StatCard label="设备在线率" :value="onlineRate + '%'" :trend="`告警 ${recentAlarms.length}`" :trendTone="recentAlarms.length > 0 ? 'warning' : 'neutral'" valueTone="primary" />
    </section>

    <section class="gb-grid gb-grid--4col">
      <article class="gb-card gb-card--chart">
        <header class="gb-card-title">
          <span>CPU 使用率</span>
          <span class="meta" :style="{ color: cpuToneColor }">{{ cpuPercent }}% · 峰值 {{ cpuPeak }}%</span>
        </header>
        <div class="traffic-svg">
          <svg viewBox="0 0 600 180" preserveAspectRatio="none">
            <defs>
              <linearGradient :id="gradIds.cpu" x1="0" x2="0" y1="0" y2="1">
                <stop offset="0%" stop-color="var(--state-warning)" stop-opacity="0.4" />
                <stop offset="100%" stop-color="var(--state-warning)" stop-opacity="0" />
              </linearGradient>
            </defs>
            <path :d="cpuArea" :fill="`url(#${gradIds.cpu})`" />
            <path :d="cpuLine" stroke="var(--state-warning)" stroke-width="1.5" fill="none" />
            <line x1="0" :y1="cpuThresholdY" x2="600" :y2="cpuThresholdY" stroke="var(--state-error)" stroke-width="1" stroke-dasharray="4 4" opacity="0.55" />
          </svg>
          <div class="legend">
            <span><i style="background: var(--state-warning)" />当前 {{ cpuPercent }}%</span>
            <span class="meta">采样 {{ cpuSamples }} 点 · 阈值 80%</span>
          </div>
        </div>
      </article>

      <article class="gb-card gb-card--chart">
        <header class="gb-card-title">
          <span>内存使用率</span>
          <span class="meta" :style="{ color: memToneColor }">{{ memPercent }}% · 峰值 {{ memPeak }}%</span>
        </header>
        <div class="traffic-svg">
          <svg viewBox="0 0 600 180" preserveAspectRatio="none">
            <defs>
              <linearGradient :id="gradIds.mem" x1="0" x2="0" y1="0" y2="1">
                <stop offset="0%" stop-color="var(--state-success)" stop-opacity="0.4" />
                <stop offset="100%" stop-color="var(--state-success)" stop-opacity="0" />
              </linearGradient>
            </defs>
            <path :d="memArea" :fill="`url(#${gradIds.mem})`" />
            <path :d="memLine" stroke="var(--state-success)" stroke-width="1.5" fill="none" />
            <line x1="0" :y1="memThresholdY" x2="600" :y2="memThresholdY" stroke="var(--state-warning)" stroke-width="1" stroke-dasharray="4 4" opacity="0.55" />
          </svg>
          <div class="legend">
            <span><i style="background: var(--state-success)" />当前 {{ memPercent }}%</span>
            <span class="meta">{{ memUsedGb }} / {{ memTotalGb }} GB · 阈值 80%</span>
          </div>
        </div>
      </article>

      <article class="gb-card gb-card--chart">
        <header class="gb-card-title">
          <span>网络流量 (Mbps)</span>
          <span class="meta">入向 ↓ / 出向 ↑</span>
        </header>
        <div class="traffic-svg">
          <svg viewBox="0 0 600 180" preserveAspectRatio="none">
            <defs>
              <linearGradient :id="gradIds.netIn" x1="0" x2="0" y1="0" y2="1">
                <stop offset="0%" stop-color="var(--brand-primary-400)" stop-opacity="0.4" />
                <stop offset="100%" stop-color="var(--brand-primary-400)" stop-opacity="0" />
              </linearGradient>
              <linearGradient :id="gradIds.netOut" x1="0" x2="0" y1="0" y2="1">
                <stop offset="0%" stop-color="var(--state-error)" stop-opacity="0.4" />
                <stop offset="100%" stop-color="var(--state-error)" stop-opacity="0" />
              </linearGradient>
            </defs>
            <path :d="trafficIn" :fill="`url(#${gradIds.netIn})`" />
            <path :d="trafficInLine" stroke="var(--brand-primary-500)" stroke-width="1.5" fill="none" />
            <path :d="trafficOut" :fill="`url(#${gradIds.netOut})`" />
            <path :d="trafficOutLine" stroke="var(--state-error)" stroke-width="1.5" fill="none" />
          </svg>
          <div class="legend">
            <span><i style="background: var(--brand-primary-500)" />入向 {{ trafficInCurrent }} Mbps</span>
            <span><i style="background: var(--state-error)" />出向 {{ trafficOutCurrent }} Mbps</span>
            <span class="meta">峰值 {{ trafficYMax }} Mbps</span>
          </div>
        </div>
      </article>

      <article class="gb-card gb-card--chart">
        <header class="gb-card-title">
          <span>系统负载 (load average)</span>
          <span class="meta">1m {{ loadAvg1 }} · 5m {{ loadAvg5 }} · 15m {{ loadAvg15 }}</span>
        </header>
        <div class="traffic-svg">
          <svg viewBox="0 0 600 180" preserveAspectRatio="none">
            <defs>
              <linearGradient :id="gradIds.load1" x1="0" x2="0" y1="0" y2="1">
                <stop offset="0%" stop-color="var(--brand-primary-400)" stop-opacity="0.4" />
                <stop offset="100%" stop-color="var(--brand-primary-400)" stop-opacity="0" />
              </linearGradient>
            </defs>
            <path :d="loadArea1" :fill="`url(#${gradIds.load1})`" />
            <path :d="loadLine1" stroke="var(--brand-primary-500)" stroke-width="1.5" fill="none" />
            <path :d="loadLine5" stroke="var(--state-warning)" stroke-width="1" fill="none" stroke-dasharray="3 3" />
            <path :d="loadLine15" stroke="var(--text-tertiary)" stroke-width="1" fill="none" stroke-dasharray="2 4" />
            <line x1="0" :y1="loadCoreMarkY" x2="600" :y2="loadCoreMarkY" stroke="var(--state-error)" stroke-width="1" stroke-dasharray="4 4" opacity="0.45" />
          </svg>
          <div class="legend">
            <span><i style="background: var(--brand-primary-500)" />1m {{ loadAvg1 }}</span>
            <span><i style="background: var(--state-warning)" />5m {{ loadAvg5 }}</span>
            <span><i style="background: var(--text-tertiary)" />15m {{ loadAvg15 }}</span>
            <span class="meta">阈值 = CPU 核心数 {{ cpuCores }}</span>
          </div>
        </div>
      </article>
    </section>

    <section class="gb-grid gb-grid--4col">

      <article class="gb-card gb-card--chart">
        <header class="gb-card-title">
          <span>服务健康</span>
          <span class="meta">实时状态</span>
        </header>
        <ul class="health-grid">
          <li v-for="s in healthList" :key="s.key" class="health-row">
            <el-icon class="health-icon" :size="18"><component :is="s.icon" /></el-icon>
            <span class="health-name">{{ s.label }}</span>
            <span class="health-detail text-tertiary text-xs">{{ s.detail }}</span>
            <span :class="['health-pill', s.tone]">
              <i class="health-dot" /> {{ s.status }}
            </span>
          </li>
        </ul>
      </article>

      <article class="gb-card gb-card--chart">
        <header class="gb-card-title">
          <span>磁盘使用率</span>
          <span class="meta">{{ diskMountList.length }} / {{ diskMounts }} 个挂载点（≥1GB 真实磁盘）· 阈值 80%</span>
        </header>
        <div class="disk-bars">
          <div v-for="(m, i) in diskMountList" :key="m.path + i" class="disk-row">
            <span class="disk-path" :title="m.path">{{ shortPath(m.path) }}</span>
            <div class="disk-track">
              <div class="disk-fill" :style="{ width: Math.min(m.pct, 100) + '%' }" :class="diskTone(m.pct)" />
              <div class="disk-mark" />
            </div>
            <span class="disk-pct mono" :class="diskTone(m.pct)">{{ m.pct }}%</span>
            <span class="disk-size text-tertiary text-xs">{{ m.usedGb }} / {{ m.totalGb }} GB</span>
          </div>
          <div v-if="diskMountList.length === 0" class="disk-empty text-tertiary text-xs">暂无磁盘数据</div>
        </div>
      </article>

      <article class="gb-card gb-card--chart">
        <header class="gb-card-title">
          <span>节点负载 Top 5</span>
          <button class="gb-btn-link" @click="goMedia">查看全部</button>
        </header>
        <div class="native-tbl-wrap">
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
              <tr v-if="!nodes.length">
                <td colspan="5" class="text-tertiary text-xs" style="text-align:center">暂无节点数据</td>
              </tr>
            </tbody>
          </table>
        </div>
      </article>

      <article class="gb-card gb-card--chart">
        <header class="gb-card-title">
          <span>最近告警</span>
          <button class="gb-btn-link" @click="goAlarm">查看告警</button>
        </header>
        <ul class="alarms">
          <li v-for="a in recentAlarms" :key="a.id" class="alarm">
            <span :class="['gb-dot', toneLevel(a.alarmPriority)]" />
            <div class="flex-1">
              <div class="text-sm text-bold">{{ a.alarmDescription ?? a.deviceId }}</div>
              <div class="text-xs text-tertiary">{{ a.deviceId }} · {{ a.alarmTime }}</div>
            </div>
            <span :class="['gb-chip', 'gb-chip--' + toneLevel(a.alarmPriority)]">
              {{ alarmPriorityLabel(a.alarmPriority) }}
            </span>
          </li>
          <li v-if="!recentAlarms.length" class="alarm text-tertiary text-xs">暂无告警</li>
        </ul>
      </article>

    </section>

    <section class="gb-card">
      <header class="gb-card-title">
        <span>重点通道</span>
        <span class="meta">点击播放预览</span>
      </header>
      <div
        v-if="channels.length"
        class="gb-grid"
        style="grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); padding: 14px;"
      >
        <VideoCell
          v-for="cell in channels"
          :key="cell.deviceId + '_' + cell.channelId"
          v-bind="cell"
          @click="onCellClick(cell)"
        />
      </div>
      <EmptyState v-else text="暂无正在拉流的通道，去「实时直播」点播后这里会显示缩略图" />
    </section>


    <!-- 通道播放对话框：取代跳转 /live -->
    <ChannelPlayDialog v-model="playVisible" :channel="playingChannel" />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import {
  Calendar,
  Connection,
  Cpu,
  DataLine,
  DocumentCopy,
  Histogram,
  VideoCamera,
  VideoPlay
} from '@element-plus/icons-vue'
import { useRouter } from 'vue-router'
import StatCard from '@/components/StatCard/index.vue'
import VideoCell from '@/components/VideoCell/index.vue'
import EmptyState from '@/components/EmptyState/index.vue'
import ChannelPlayDialog from '@/components/ChannelPlayDialog/index.vue'
import type { SystemInfo } from '@/api/log'
import { loadSystemInfo } from '@/utils/systemInfo'
import { queryDevices } from '@/api/device'
import { listSnapshots, queryStreams, snapshotKey, type MediaStreamRow } from '@/api/live'
import { dedupeMediaStreams, extractKeyChannels } from '@/utils/mediaStream'
import { getMediaServerList, getMediaLoad } from '@/api/mediaServer'
import { getAlarmList, alarmPriorityLabel } from '@/api/alarm'

const router = useRouter()

// 自动刷新间隔
const REFRESH_INTERVAL_MS = 2_000
let refreshTimer: number | null = null

const info = ref<SystemInfo>({})
const deviceTotal = ref(0)
const deviceOnline = ref(0)
const channelTotal = ref(0)
const activeStreamCount = ref(0)
const streams = ref<MediaStreamRow[]>([])
const mediaServerCount = ref(0)
const recentAlarms = ref<{
  id?: number
  alarmTime?: string
  alarmDescription?: string
  deviceId?: string
  alarmPriority?: string
}[]>([])
const mediaServerOnlineCount = ref(0)
const nodes = ref<{ id: string; name: string; region: string; cpu: number; mem: number; bw: number; status: string; tone: string }[]>([])
const channels = ref<Array<{ id: number; title: string; no: string; state: 'live' | 'rec' | 'mute' | 'offline'; deviceId?: string; channelId?: string; thumb?: string }>>([])

/**
 * 节点列表与节点流量的最近一次结果。它们不直接渲染，而是由 `applyNodeRows()`
 * 和 `system_info` 的 CPU/内存合成节点卡片 —— 两个接口谁后到都要重算一次，
 * 所以要留在模块作用域里（放局部变量时，system_info 到达后就再也补不上了）。
 */
type MediaServerRow = { id?: string; ip?: string; httpPort?: number; status?: boolean }
let mediaServers: MediaServerRow[] = []
let nodeLoad = new Map<string, any>()

// 通道播放对话框（重点通道点击 → 弹出播放）
const playVisible = ref(false)
const playingChannel = ref<{ deviceId: string; channelId: string; name: string } | null>(null)

// 三个曲线面板的 gradient id 必须全局唯一（SVG <defs> 在同一文档里复用），
// 用面板下标做后缀避免互相覆盖。
const gradIds = {
  cpu: `gb-cpu-${Math.random().toString(36).slice(2, 8)}`,
  mem: `gb-mem-${Math.random().toString(36).slice(2, 8)}`,
  netIn: `gb-net-in-${Math.random().toString(36).slice(2, 8)}`,
  netOut: `gb-net-out-${Math.random().toString(36).slice(2, 8)}`,
  load1: `gb-load1-${Math.random().toString(36).slice(2, 8)}`,
  disk: `gb-disk-${Math.random().toString(36).slice(2, 8)}`
}

const channelOnline = ref<number | undefined>(undefined)

const loading = ref(false)
const lastSyncAt = ref<number>(Date.now())
const now = ref<number>(Date.now())
const syncLabel = computed(() => {
  const ms = now.value - lastSyncAt.value
  if (ms < 5_000) return '刚刚'
  if (ms < 60_000) return `${Math.floor(ms / 1000)} 秒前`
  return `${Math.floor(ms / 60_000)} 分钟前`
})
// 1s 心跳让"最近同步"标签实时刷新（10s 自动轮询之间有节奏感）
let syncTicker: number | null = null

// 服务健康状态：从最近一次 loadAll 的 API 状态推断。
// 后端子系统（Postgres/Redis/SIP/JT1078/录制计划）的健康由对应 API 成功
// 推断；ZLM 节点状态取自 mediaServerList。
const apiOk = ref<{ sys: boolean; devs: boolean; streams: boolean; media: boolean; alarms: boolean }>({
  sys: false,
  devs: false,
  streams: false,
  media: false,
  alarms: false
})

/**
 * 控制台首屏数据装载。
 *
 * **每个面板各写各的**：5 个接口谁先回来谁先渲染，不再"等最慢的那个回来再统一赋值"。
 *
 * 为什么改：`/api/server/system/info` 在服务端要真采一次 CPU（60ms）与网络速率
 * （2×100ms）+ 读磁盘，本机单次实测约 0.8s，浏览器里叠加侧边栏/导航栏的同名请求
 * 常到 1.2~1.5s。此前是 `await Promise.allSettled([5 个接口])` 之后才逐项赋值，
 * 于是 0.5s 就返回的设备数 / 流列表 / 告警 / 媒体节点也要陪着它一起等 ——
 * 表现就是"打开控制台要等约 2s 才出数据"。
 */
async function loadAll() {
  loading.value = true
  lastSyncAt.value = Date.now()
  try {
    await Promise.allSettled([
      loadSystemInfo().then((data) => {
        // 失败时保持上一次的值（不要把界面清成空）
        if (data) {
          info.value = data
          apiOk.value.sys = true
          applyInfoDerived()
        } else {
          apiOk.value.sys = false
        }
      }),
      queryDevices({ page: 1, count: 1 })
        .then((devs) => {
          deviceTotal.value = devs.data?.total ?? 0
          apiOk.value.devs = true
        })
        .catch(() => {
          apiOk.value.devs = false
        }),
      queryStreams({ page: 1, count: 1000 })
        .then((res) => {
          const list = ((res.data as any)?.list ?? []) as MediaStreamRow[]
          streams.value = list
          // ZLM 对同一路流按协议各返回一行（rtsp/rtmp/hls/ts/fmp4），原始 list 的长度是
          // 「行数」而不是「路数」：不去重的话卡片右侧的「直播 N」会虚高好几倍。
          activeStreamCount.value = dedupeMediaStreams(list).length
          apiOk.value.streams = true
          // 流列表一到就重建重点通道，不必再等 system/info
          rebuildChannels()
          applyInfoDerived()
        })
        .catch(() => {
          apiOk.value.streams = false
        }),
      getMediaServerList()
        .then(async (mss) => {
          mediaServers = ((mss.data as any[]) ?? []) as MediaServerRow[]
          mediaServerCount.value = mediaServers.length
          mediaServerOnlineCount.value = mediaServers.filter((m) => m.status === true).length
          apiOk.value.media = true
          // 节点流量只依赖节点列表，跟在它后面查即可，别被 system/info 拖住
          await loadNodeTraffic()
          applyNodeRows()
        })
        .catch(() => {
          apiOk.value.media = false
        }),
      getAlarmList({ page: 1, count: 5 })
        .then((alarms) => {
          recentAlarms.value = alarms.data?.list ?? []
          apiOk.value.alarms = true
        })
        .catch(() => {
          apiOk.value.alarms = false
        })
    ])
  } finally {
    loading.value = false
  }
}

/**
 * 把 `system_info` 派生出的标量铺到界面上（通道数/在线数/设备在线数 + 节点表）。
 *
 * 之所以要单独抽出来：这些值分别来自 `system/info` 与 `device/query/streams`，
 * 两个接口谁先回来都要能用**当前已知的另一半**把界面刷新一次。
 */
function applyInfoDerived() {
  // system_info 给的 channelTotal 是 DB 里的真实值（包含未在拉流的通道）；
  // 拿不到时回落到当前活跃拉流的**路数**（去重后，不是 ZLM 的协议行数）
  channelTotal.value =
    (typeof info.value.channelTotal === 'number' ? info.value.channelTotal : undefined) ??
    activeStreamCount.value
  channelOnline.value =
    typeof info.value.channelOnline === 'number' ? info.value.channelOnline : undefined
  deviceOnline.value = info.value.deviceOnline ?? 0
  applyNodeRows()
}

/** 节点列表 → 卡片。CPU/内存取自 system_info，两个接口任一到达都会重建一次。 */
function applyNodeRows() {
  const sysCpu = info.value.cpu_usage ?? 0
  const sysMem = info.value.mem_usage ?? 0
  nodes.value = mediaServers.slice(0, 6).map((m, i) => {
    const ld = nodeLoad.get(m.id ?? '')
    const bw = Math.round(((ld?.gbReceive ?? 0) + (ld?.gbSend ?? 0)) * 100) / 100
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
}

/** 拉取各媒体节点的收/发流量（最多 6 个节点）。 */
async function loadNodeTraffic() {
  const picked = mediaServers.slice(0, 6)
  nodeLoad = new Map<string, any>()
  const loadRes = await Promise.allSettled(picked.map((m) => getMediaLoad(m.id ?? '')))
  for (let i = 0; i < picked.length && i < loadRes.length; i++) {
    const r = loadRes[i]
    if (r.status !== 'fulfilled') continue
    // 按 id 取自己那一项：后端返回的是数组，此前取 `arr[0]`
    // 会把第一个节点的流量显示到每一张卡片上
    const arr = (r.value.data as any[]) ?? []
    const item = arr.find((x) => x?.id === picked[i].id) ?? arr[0]
    if (item) nodeLoad.set(picked[i].id ?? '', item)
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

// ---- CPU / 内存：实时百分比（0-100），曲线面板用 ----
const cpuPercent = computed(() => {
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

// ---- CPU / 内存 曲线（与流量图同一套 600x180 viewBox） ----
// 折线 + 区域填充；阈值用 80% 虚线提示。Y 轴固定 0-100。
const PERCENT_WINDOW = 60 // 多于 60 个点时只画末尾一段，保持密度稳定
const cpuSeries = computed<number[]>(() => {
  const arr = (info.value.cpu ?? []) as Array<{ data?: number }>
  return arr.slice(-PERCENT_WINDOW).map((p) => Math.round((Number(p.data ?? 0)) * 100))
})
const memSeries = computed<number[]>(() => {
  // 后端 system_info.memory.mem 与 info.mem 都能用，优先 mem
  const arr = (info.value.mem as Array<{ data?: number }> | undefined) ?? []
  return arr.slice(-PERCENT_WINDOW).map((p) => Math.round((Number(p.data ?? 0)) * 100))
})
const cpuPeak = computed(() => Math.max(0, ...cpuSeries.value))
const memPeak = computed(() => Math.max(0, ...memSeries.value))
const cpuSamples = computed(() => cpuSeries.value.length)
const memSamples = computed(() => memSeries.value.length)

// viewBox 是 600 x 180，留 20 顶部 + 20 底部 padding
const CHART_W = 600
const CHART_H = 180
const CHART_PAD = 20
const CHART_DRAW_H = CHART_H - CHART_PAD // 160
const buildAreaPath = (arr: number[], max: number) => {
  if (arr.length === 0) return ''
  const w = CHART_W / Math.max(arr.length - 1, 1)
  const yOf = (v: number) => CHART_H - (Math.min(v, max) / max) * CHART_DRAW_H - 4
  const line = arr
    .map((v, i) => `${i === 0 ? 'M' : 'L'} ${i * w} ${yOf(v)}`)
    .join(' ')
  return `${line} L ${CHART_W} ${CHART_H} L 0 ${CHART_H} Z`
}
const buildLinePath = (arr: number[], max: number) => {
  if (arr.length === 0) return ''
  const w = CHART_W / Math.max(arr.length - 1, 1)
  return arr
    .map((v, i) => `${i === 0 ? 'M' : 'L'} ${i * w} ${CHART_H - (Math.min(v, max) / max) * CHART_DRAW_H - 4}`)
    .join(' ')
}
const yOfThreshold = (pct: number) => CHART_H - (Math.min(pct, 100) / 100) * CHART_DRAW_H - 4
const cpuArea = computed(() => buildAreaPath(cpuSeries.value, 100))
const cpuLine = computed(() => buildLinePath(cpuSeries.value, 100))
const memArea = computed(() => buildAreaPath(memSeries.value, 100))
const memLine = computed(() => buildLinePath(memSeries.value, 100))
const cpuThresholdY = computed(() => yOfThreshold(80))
const memThresholdY = computed(() => yOfThreshold(80))

// 卡片标题颜色随当前值变化：绿→黄→红
const cpuToneColor = computed(() => {
  if (cpuPercent.value >= 90) return 'var(--state-error)'
  if (cpuPercent.value >= 70) return 'var(--state-warning)'
  return 'var(--text-primary)'
})
const memToneColor = computed(() => {
  if (memPercent.value >= 90) return 'var(--state-error)'
  if (memPercent.value >= 80) return 'var(--state-warning)'
  return 'var(--text-primary)'
})

// 内存用量：bytes → GB（保留 1 位小数）
const memUsedGb = computed(() => {
  const used = info.value.memory?.used ?? 0
  return (Number(used) / 2 ** 30).toFixed(1)
})
const memTotalGb = computed(() => {
  const total = info.value.memory?.total ?? 0
  return (Number(total) / 2 ** 30).toFixed(1)
})

// ---- 系统负载（load average 1/5/15 min） ----
// load_avg 标量来自后端；load[] ring buffer 只存 1m 曲线（5/15 取当前值画
// 虚线水平线足够展示）。
const cpuCores = computed(() => (typeof navigator !== 'undefined' && (navigator as any).hardwareConcurrency) || 4)
const loadAvg1 = computed(() => {
  const la = info.value.load_avg as { '1'?: number; '5'?: number; '15'?: number } | null | undefined
  return Number(la?.['1'] ?? 0).toFixed(2)
})
const loadAvg5 = computed(() => {
  const la = info.value.load_avg as { '1'?: number; '5'?: number; '15'?: number } | null | undefined
  return Number(la?.['5'] ?? 0).toFixed(2)
})
const loadAvg15 = computed(() => {
  const la = info.value.load_avg as { '1'?: number; '5'?: number; '15'?: number } | null | undefined
  return Number(la?.['15'] ?? 0).toFixed(2)
})
const loadSeries1 = computed<number[]>(() => {
  const arr = (info.value.load ?? []) as Array<{ data?: number }>
  return arr.slice(-PERCENT_WINDOW).map((p) => Number(p.data ?? 0))
})
// 5m / 15m 没有 ring buffer，用当前 load_avg 值画一条水平虚线（通过
// 把单值映射成全数组，长度跟 loadSeries1 同步）
const loadSeries5 = computed<number[]>(() => {
  const len = loadSeries1.value.length || 1
  return Array(len).fill(Number(loadAvg5.value))
})
const loadSeries15 = computed<number[]>(() => {
  const len = loadSeries1.value.length || 1
  return Array(len).fill(Number(loadAvg15.value))
})
// Y 轴上限：取核心数与峰值中较大的，再加 20% 留白
const loadYMax = computed(() => {
  const peak = Math.max(...loadSeries1.value, cpuCores.value, 1)
  return Math.max(Math.ceil(peak * 1.2), cpuCores.value + 1)
})
const loadArea1 = computed(() => buildAreaPath(loadSeries1.value, loadYMax.value))
const loadLine1 = computed(() => buildLinePath(loadSeries1.value, loadYMax.value))
const loadLine5 = computed(() => buildLinePath(loadSeries5.value, loadYMax.value))
const loadLine15 = computed(() => buildLinePath(loadSeries15.value, loadYMax.value))
const loadCoreMarkY = computed(() => yOfThreshold((cpuCores.value / loadYMax.value) * 100))

// ---- 磁盘 ----
// 磁盘面板用横向柱状图（每根 = 一个真实挂载点）。
// 磁盘面板过滤：只显示大容量的真实磁盘。
//
// 过滤规则（任一不过滤都丢弃）：
//   1. fs 不是虚拟文件系统（tmpfs / devtmpfs / efivarfs / overlay /
//      squashfs / aufs / proc / sysfs / cgroup* / debugfs / tracefs /
//      ramfs / devpts / fusectl / configfs / ...）
//   2. total >= 1 GB（小于 1GB 的 EFI 分区、挂载的小卷都丢掉）
const MIN_DISK_BYTES = 1024 ** 3 // 1 GB
const VIRTUAL_FS = new Set([
  'tmpfs', 'devtmpfs', 'efivarfs', 'overlay', 'overlayfs',
  'squashfs', 'aufs', 'proc', 'sysfs', 'devpts', 'securityfs',
  'selinuxfs', 'binfmt_misc', 'hugetlbfs', 'mqueue', 'pstore',
  'ramfs', 'fusectl', 'configfs', 'debugfs', 'tracefs',
  'fuse.gvfsd-fuse', 'rpc_pipefs', 'nsfs', 'autofs',
  'bpf', 'cgroup', 'cgroup2'
])
function isRealDiskFs(fs: string | undefined): boolean {
  if (!fs) return true // 未知 fs 不过滤（保守）
  const f = fs.toLowerCase()
  return !VIRTUAL_FS.has(f)
}

const diskRawList = computed(() => {
  const arr = (info.value.disk_all ?? info.value.disk ?? []) as Array<unknown>
  return arr as Array<{ fs?: string; path?: string; used?: number; total?: number }>
})
const diskMounts = computed(() => diskRawList.value.length)
const diskMountList = computed<Array<{ path: string; pct: number; usedGb: string; totalGb: string }>>(() => {
  return diskRawList.value
    .filter((d) => isRealDiskFs(d.fs) && Number(d.total ?? 0) >= MIN_DISK_BYTES)
    .map((d) => {
      const total = Number(d.total ?? 0)
      const used = Number(d.used ?? 0)
      const pct = total > 0 ? Math.round((used / total) * 100) : 0
      return {
        path: d.path ?? '/',
        pct,
        usedGb: (used / 2 ** 30).toFixed(1),
        totalGb: (total / 2 ** 30).toFixed(1)
      }
    })
    .sort((a, b) => b.pct - a.pct) // 最满的排前面
})
function shortPath(p: string): string {
  if (!p) return '/'
  // 截断长路径：超过 16 字符用 … 代替中间
  if (p.length <= 16) return p
  return p.slice(0, 8) + '…' + p.slice(-6)
}
function diskTone(pct: number): string {
  if (pct >= 90) return 'tone-error'
  if (pct >= 80) return 'tone-warning'
  return 'tone-ok'
}

// ---- 服务健康（图标 + 状态 pill） ----
interface HealthRow {
  key: string
  label: string
  icon: any
  detail: string
  status: string
  tone: 'ok' | 'warn' | 'err'
}
// 协议接入（SIP / JT1078）的展示已挪到顶栏「平台信息」弹层
// （components/PlatformInfo），控制台不再持有这部分数据与复制逻辑。

const healthList = computed<HealthRow[]>(() => {
  const ok = apiOk.value
  const total = mediaServerCount.value
  const on = mediaServerOnlineCount.value
  return [
    {
      key: 'zlm',
      label: '媒体节点',
      icon: VideoCamera,
      detail: `${on} / ${total} 在线`,
      status: ok.media && total > 0 ? '正常' : (ok.media ? '无节点' : '离线'),
      tone: ok.media && total > 0 && on === total ? 'ok' : (ok.media ? 'warn' : 'err')
    },
    {
      key: 'db',
      label: '数据库',
      icon: Histogram,
      detail: ok.devs ? `${deviceTotal.value} 设备` : '查询失败',
      status: ok.devs ? '正常' : '离线',
      tone: ok.devs ? 'ok' : 'err'
    },
    {
      key: 'redis',
      label: 'Redis',
      icon: DataLine,
      detail: ok.sys ? '已连接' : '未连接',
      status: ok.sys ? '正常' : '离线',
      tone: ok.sys ? 'ok' : 'err'
    },
    {
      key: 'sip',
      label: 'GB28181',
      icon: Connection,
      detail: 'UDP/TCP :5060',
      status: ok.sys ? '监听中' : '离线',
      tone: ok.sys ? 'ok' : 'err'
    },
    {
      key: 'jt1078',
      label: 'JT1078',
      icon: VideoPlay,
      detail: 'TCP/UDP :60000',
      status: ok.sys ? '监听中' : '离线',
      tone: ok.sys ? 'ok' : 'err'
    },
    {
      key: 'record',
      label: '录制计划',
      icon: Calendar,
      detail: ok.sys ? '调度器运行中' : '未启动',
      status: ok.sys ? '正常' : '离线',
      tone: ok.sys ? 'ok' : 'err'
    }
  ]
})

const onlineRate = computed(() => deviceTotal.value ? Math.round((deviceOnline.value / deviceTotal.value) * 100) : 0)

// 网络流量图：用 system_info.net 数组（真实 in/out Mbps 时间序列）。
// 历史不足 12 个点时取全部；多于 12 时取末尾 12 个保持曲线密度稳定。
const NET_WINDOW = 12
const trafficInArr = computed<number[]>(() => {
  const net = (info.value.net ?? []) as Array<{ in?: number; out?: number }>
  const series = net.slice(-NET_WINDOW).map((p) => Number(p.in ?? 0))
  // 没有历史时画一根平线，比空白更直观
  if (series.length === 0) return [0]
  return series
})
const trafficOutArr = computed<number[]>(() => {
  const net = (info.value.net ?? []) as Array<{ in?: number; out?: number }>
  const series = net.slice(-NET_WINDOW).map((p) => Number(p.out ?? 0))
  if (series.length === 0) return [0]
  return series
})
// 给 sparkline 用一份统一的"当前活跃度"序列：取 in/out 最大值，
// 不再是 sin 假数据。
const trafficArr = computed(() =>
  trafficInArr.value.map((v, i) => Math.max(v, trafficOutArr.value[i] ?? 0))
)
const t = computed(() => trafficInArr.value)
const o = computed(() => trafficOutArr.value)
const trafficYMax = computed(() => {
  // y 轴上限：实际峰值向上取整到 1 Mbps；零流量时给 1 Mbps 兜底
  const peak = Math.max(...t.value, ...o.value, 0)
  if (peak <= 0) return 1
  return Math.ceil(peak)
})
const trafficUnit = computed(() => (trafficYMax.value >= 1 ? 'Mbps' : 'Mbps'))
// 当前入/出向流量：取 network[] 末项（实时聚合），没有则退到 net[] 末项
const trafficInCurrent = computed(() => {
  const n = (info.value.network as Array<{ rx?: number }> | undefined)?.[0]
  if (n?.rx != null) return Number(n.rx.toFixed(2))
  return Number((t.value[t.value.length - 1] ?? 0).toFixed(2))
})
const trafficOutCurrent = computed(() => {
  const n = (info.value.network as Array<{ tx?: number }> | undefined)?.[0]
  if (n?.tx != null) return Number(n.tx.toFixed(2))
  return Number((o.value[o.value.length - 1] ?? 0).toFixed(2))
})
// statCard sparklines —— 设备/通道/在线率用 mem 历史；媒体节点用网络入向 Mbps * 10。
// 4 个 KPI 卡片之前挂过 spark 线（用 mem/net 数据当代理画在设备/通道/在线率下面），
// 但这些 spark **与 KPI 数值没有任何真实关联**，只是"系统活跃度"的代理曲线，
// 用户看着误以为是"在线设备数变化趋势"。需求要求去掉 —— 已不再传 :spark。
const make = (arr: number[]) => {
  const max = trafficYMax.value
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
/**
 * 告警级别 → 色调。入参是后端的 `alarmPriority`（GB/T 28181 报警优先级：
 * 1 一级/紧急、2 二级/重要、3 三级/一般、4 四级/提示），
 * 同时兼容历史数据里可能出现的文字值。
 */
function toneLevel(priority?: string | number | null): string {
  const lv = String(priority ?? '').toUpperCase()
  if (lv === '1' || lv.includes('紧急') || lv === 'ERROR' || lv === 'CRITICAL') return 'error'
  if (lv === '2' || lv.includes('警告') || lv === 'WARN' || lv === 'WARNING') return 'warning'
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
  // 重点通道点击不再跳 /live —— 直接弹播放对话框，留在仪表盘
  playingChannel.value = {
    deviceId: c.deviceId,
    channelId: c.channelId,
    name: c.title ?? c.channelId
  }
  playVisible.value = true
}

onMounted(async () => {
  // 首屏立刻发请求（loadAll 内部各面板各自落地，不互相等待）；
  // 重点通道由流列表那一支自己重建，这里不需要再等一轮。
  await loadAll()
  // 启动自动刷新（2s 周期与后端 health_check 默认值对齐）。
  // 切页或组件卸载时由 onBeforeUnmount 清掉，避免泄漏。
  refreshTimer = window.setInterval(() => {
    loadAll()
  }, REFRESH_INTERVAL_MS)
  // 1s 心跳让"最近同步：x 秒前"标签实时走动
  syncTicker = window.setInterval(() => {
    now.value = Date.now()
  }, 1000)
})

function rebuildChannels() {
  // 面板是「通道」口径，不能直接拿 ZLM 的原始行铺格子：
  //  1) ZLM 对同一路流按协议各返回一行（rtsp/rtmp/hls/ts/fmp4），原始行数 ≈ 路数 × 5；
  //  2) 同一通道可能同时有实时流与回放/下载流。
  // extractKeyChannels 会依次消掉这两层倍数 —— 同一通道在面板上只占一个格子。
  const picked = extractKeyChannels(streams.value, 6)
  // 轮询重建时沿用上一次的缩略图，避免每 2s 闪一下占位符
  const prevThumb = new Map(
    channels.value.map((c) => [`${c.deviceId}_${c.channelId}`, c.thumb ?? ''])
  )
  channels.value = picked.map((c, i) => ({
    id: i + 1,
    title: c.stream || c.channelId,
    no: `C${String(i + 1).padStart(3, '0')}`,
    // 只有回放/下载流在跑的通道标 REC，不再一律显示 LIVE
    state: c.live ? ('live' as const) : ('rec' as const),
    deviceId: c.deviceId,
    // 必须用后端解析出的 channelId（国标通道号），此前误用 ZLM 的流名，
    // 跳转过去必然找不到通道
    channelId: c.channelId,
    thumb: prevThumb.get(c.key) ?? ''
  }))
  if (channels.value.length > 0) {
    // 异步给每张卡片抓一帧缩略图；失败的不影响主流程
    refreshSnaps()
  }
}

/**
 * 给"重点通道"每个格子铺上**已保存的**缩略图。
 *
 * 缩略图由后端在点播时自动抓帧落盘，这里只批量读存量 —— 一次请求拿到
 * 全部格子的图，不碰 ZLM、不唤醒设备。没被点播过的通道保持占位符。
 */
async function refreshSnaps() {
  const keys = channels.value
    .filter((c) => c.deviceId && c.channelId)
    .map((c) => snapshotKey(c.deviceId!, c.channelId!))
  if (keys.length === 0) return
  try {
    const res = await listSnapshots(keys)
    const map = res?.data ?? {}
    for (const c of channels.value) {
      if (!c.deviceId || !c.channelId) continue
      const url = map[snapshotKey(c.deviceId, c.channelId)]
      if (url) c.thumb = url
    }
  } catch {
    // 静默：拉不到图保留占位符
  }
}

onBeforeUnmount(() => {
  if (refreshTimer !== null) {
    window.clearInterval(refreshTimer)
    refreshTimer = null
  }
  if (syncTicker !== null) {
    window.clearInterval(syncTicker)
    syncTicker = null
  }
})
</script>

<style lang="scss" scoped>
.traffic-svg { padding: 14px 18px; }
.traffic-svg svg { width: 100%; height: 180px; display: block; }
.legend {
  display: flex; gap: 16px; font-size: var(--text-xs); color: var(--text-tertiary);
  margin-top: 6px;
  flex-wrap: wrap;
  i { width: 10px; height: 2px; display: inline-block; margin-right: 4px; vertical-align: middle; }
}
.gb-card--chart .traffic-svg { padding: 10px 14px; }
.gb-card--chart .traffic-svg svg { width: 100%; height: 180px; display: block; }
/* 让卡片成为 flex column，子项（标题 / 图表 / 图例）可以伸缩，
   这样磁盘柱条列表就能撑满到兄弟卡片（协议接入）的高度。 */
.gb-card--chart { display: flex; flex-direction: column; }
.meta { color: var(--text-tertiary); }

/* ---- 磁盘横排柱状图 ---- */
.disk-bars {
  padding: 10px 14px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  /* 撑满父容器的高度（同行的服务健康 / 协议接入可能更高，磁盘
     跟着 grid 子项的最大高度走）；挂载点多时内部滚动。 */
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
}
.disk-row {
  display: grid;
  /* 磁盘行要能塞进 1/4 宽的卡片（内容区 ~200px）：原来固定
     88+38+96 = 222px 的列宽会把进度条压成 0。改成"路径可缩短、
     进度条保底 36px、数值列收紧"。 */
  grid-template-columns: minmax(40px, 72px) minmax(36px, 1fr) 32px 64px;
  gap: 6px;
  align-items: center;
  font-size: var(--text-xs);
  padding: 2px 0;
}
.disk-path { font-family: var(--font-mono); color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.disk-track {
  position: relative;
  height: 12px;
  background: var(--bg-overlay);
  border-radius: 999px;
  overflow: hidden;
}
.disk-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--state-success);
  transition: width 0.4s ease, background 0.2s;
}
.disk-fill.tone-warning { background: var(--state-warning); }
.disk-fill.tone-error { background: var(--state-error); }
.disk-mark {
  position: absolute;
  top: -2px; bottom: -2px;
  left: 80%;
  width: 2px;
  background: var(--state-warning);
  opacity: 0.55;
}
.disk-pct { font-weight: 600; color: var(--text-primary); text-align: right; }
.disk-pct.tone-warning { color: var(--state-warning); }
.disk-pct.tone-error { color: var(--state-error); }
.disk-size { text-align: right; }
.disk-empty { padding: 30px 0; text-align: center; }

/* ---- 服务健康图标格 ---- */
.health-grid { list-style: none; margin: 0; padding: 8px 14px 14px; display: flex; flex-direction: column; gap: 10px; min-height: 180px; }
/* 健康行同样要能在 1/4 宽度里放下：名称列收窄，detail 允许省略号 */
.health-row {
  display: grid;
  grid-template-columns: 20px minmax(52px, 68px) minmax(0, 1fr) auto;
  gap: 8px;
  align-items: center;
  font-size: var(--text-xs);
  padding: 4px 0;
}
.health-icon { color: var(--brand-primary-500); display: inline-flex; }
.health-name { color: var(--text-primary); font-weight: 500; }
/* 窄卡下 detail（"1 / 1 在线"）优先让位，不换行也不挤压状态 pill */
.health-detail { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.health-pill {
  display: inline-flex; align-items: center; gap: 5px;
  padding: 2px 10px;
  border-radius: 999px;
  font-size: var(--text-xs);
  font-weight: 500;
  background: var(--bg-overlay);
  color: var(--text-tertiary);
}
.health-dot {
  width: 6px; height: 6px; border-radius: 50%;
  background: currentColor;
  box-shadow: 0 0 0 2px rgba(255,255,255,0.05);
}
.health-pill.ok { color: var(--state-success); background: rgba(22, 163, 74, 0.10); }
.health-pill.warn { color: var(--state-warning); background: rgba(217, 119, 6, 0.10); }
.health-pill.err { color: var(--state-error); background: rgba(220, 38, 38, 0.10); }

/* 节点负载表：搬到 1/4 宽的卡片后 5 列放不下，给容器横向滚动 + 收紧内边距，
   宁可让它在卡内滚，也不要撑破整个 grid 行。 */
.native-tbl { width: 100%; border-collapse: collapse; font-size: var(--text-xs); }
.native-tbl th, .native-tbl td { padding: 8px 10px; border-bottom: 1px solid var(--border-subtle); text-align: left; white-space: nowrap; }
.native-tbl thead th { color: var(--text-tertiary); font-weight: 500; background: var(--bg-elevated); }
.native-tbl .ta-r { text-align: right; }
/* 窄卡内的表格：超出宽度时在卡内横向滚动，不撑破外层 grid */
.native-tbl-wrap { overflow-x: auto; }
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
