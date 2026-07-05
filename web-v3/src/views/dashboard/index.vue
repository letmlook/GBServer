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
      <StatCard label="在线设备" :value="1284" trend="↑ 12 较昨日" trendTone="success" :spark="[10, 22, 18, 30, 28, 36, 44, 52, 60, 80, 96, 110]" />
      <StatCard label="活跃通道" :value="13856" trend="在线率 96.4%" :spark="[44, 50, 48, 60, 70, 75, 80, 78, 88, 92, 100, 108]" />
      <StatCard label="今日录像 (TB)" :value="3.42" trendTone="warning" trend="↑ 8% 较昨日" :spark="[10, 14, 18, 22, 20, 26, 30, 34, 32, 36, 40, 44]" valueTone="warning" />
      <StatCard label="告警事件" :value="42" trendTone="error" trend="3 条紧急" :spark="[2, 4, 3, 6, 5, 9, 8, 10, 7, 12, 14, 18]" valueTone="error" />
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
              :stroke-dasharray="blueDash" :stroke-dashoffset="-green" transform="rotate(-90 60 60)" stroke-linecap="round" />
            <circle cx="60" cy="60" r="48" fill="none" stroke="var(--state-warning)" stroke-width="14"
              :stroke-dasharray="orangeDash" :stroke-dashoffset="-(green + blue)" transform="rotate(-90 60 60)" stroke-linecap="round" />
          </svg>
          <ul class="donut-legend">
            <li><i class="gb-dot gb-dot--success" />在线 <span class="mono">12,484</span></li>
            <li><i class="gb-dot gb-dot--info" />直播中 <span class="mono">1,008</span></li>
            <li><i class="gb-dot gb-dot--warning" />弱信号 <span class="mono">264</span></li>
            <li><i class="gb-dot" style="background: var(--text-disabled)" />离线 <span class="mono">100</span></li>
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
          <li v-for="a in alarms" :key="a.id" class="alarm">
            <span :class="['gb-dot', 'gb-dot--' + a.tone]" />
            <div class="flex-1">
              <div class="text-sm text-bold">{{ a.title }}</div>
              <div class="text-xs text-tertiary">{{ a.source }} · {{ a.time }}</div>
            </div>
            <span :class="['gb-chip', 'gb-chip--' + a.tone]">{{ a.level }}</span>
          </li>
        </ul>
      </article>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { ElMessage } from 'element-plus'
import { useRouter } from 'vue-router'
import StatCard from '@/components/StatCard/index.vue'
import VideoCell from '@/components/VideoCell/index.vue'

const router = useRouter()

const channels = [
  { id: 1, title: '校门 1', no: 'C001', state: 'live' as const, thumb: '' },
  { id: 2, title: '教学楼前', no: 'C002', state: 'rec' as const, thumb: '' },
  { id: 3, title: '宿舍区', no: 'C003', state: 'live' as const, thumb: '' },
  { id: 4, title: '操场全景', no: 'C004', state: 'mute' as const, thumb: '' },
  { id: 5, title: '食堂入口', no: 'C005', state: 'live' as const, thumb: '' },
  { id: 6, title: '图书馆前', no: 'C006', state: 'offline' as const, thumb: '' }
]

const nodes = [
  { name: 'node-edge-01', region: '北京·亦庄', cpu: 64, mem: 78, bw: 240, status: '正常', tone: 'success' },
  { name: 'node-edge-02', region: '北京·亦庄', cpu: 88, mem: 91, bw: 512, status: '高负载', tone: 'warning' },
  { name: 'node-edge-03', region: '上海·张江', cpu: 32, mem: 41, bw: 120, status: '正常', tone: 'success' },
  { name: 'node-edge-04', region: '深圳·南山', cpu: 95, mem: 60, bw: 360, status: '高负载', tone: 'error' },
  { name: 'node-edge-05', region: '成都·高新', cpu: 22, mem: 38, bw: 80, status: '正常', tone: 'success' }
]

const alarms = [
  { id: 1, title: '校门 1 设备断线', source: 'node-edge-01', time: '12:48:32', level: '紧急', tone: 'error' },
  { id: 2, title: '教学楼前 信号弱', source: 'node-edge-02', time: '12:42:18', level: '警告', tone: 'warning' },
  { id: 3, title: '录像存储阈值告警', source: 'node-edge-04', time: '12:30:01', level: '警告', tone: 'warning' },
  { id: 4, title: '平台注册成功', source: 'cascade-1', time: '12:25:09', level: '信息', tone: 'info' },
  { id: 5, title: '夜间巡航结束', source: 'task-cruise', time: '12:00:00', level: '信息', tone: 'info' }
]

function tone(v: number) {
  if (v >= 90) return 'bar-fill--error'
  if (v >= 70) return 'bar-fill--warning'
  return 'bar-fill--success'
}
function goMedia() { router.push('/mediaServer') }
function goAlarm() { router.push('/alarm') }
function refresh() { ElMessage.success('已刷新') }
function onCellClick(c: typeof channels[number]) {
  ElMessage.info(`打开通道：${c.title} (${c.no})`)
}

const total = 301
const C = 2 * Math.PI * 48
const green = C * (12484 / total)
const blue = C * (1008 / total)
const orange = C * (264 / total)
const gray = C * (100 / total)
const greenDash = `${green} ${C - green}`
const blueDash = `${blue} ${C - blue}`
const orangeDash = `${orange} ${C - orange}`

const t = [60, 50, 70, 65, 80, 75, 90, 100, 85, 95, 88, 110]
const o = [30, 28, 36, 40, 38, 42, 50, 55, 48, 60, 70, 65]
const make = (arr: number[]) => {
  const max = Math.max(...arr)
  const w = 600 / (arr.length - 1)
  return arr.map((v, i) => `${i === 0 ? 'M' : 'L'} ${i * w} ${180 - (v / max) * 160 - 4}`).join(' ')
}
const trafficIn = computed(() => make(t) + ' L 600 180 L 0 180 Z')
const trafficInLine = computed(() => make(t))
const trafficOut = computed(() => make(o) + ' L 600 180 L 0 180 Z')
const trafficOutLine = computed(() => make(o))
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
