<template>
  <div class="gb-page console-page">
    <!-- 顶部标题 + 时间维度 -->
    <div class="gb-page__header">
      <div>
        <h1 class="gb-page__title">今日系统总览</h1>
        <p class="gb-page__subtitle">
          {{ nowText }} · 自动刷新 <span class="mono text-primary-accent">{{ refreshSec }}s</span>
        </p>
      </div>
      <div class="gb-toolbar">
        <button v-for="r in ranges" :key="r" class="gb-tab" :class="{ 'is-active': range === r }" @click="range = r">{{ r }}</button>
      </div>
    </div>

    <!-- KPI 卡片 -->
    <section class="gb-grid gb-grid--kpi">
      <stat-card label="通道总数" :value="3841" value-tone="default" trend="↑ 4.2% 较昨日" trend-tone="success" :spark="[20,22,18,24,26,30,28,32,36]" />
      <stat-card label="在线设备" :value="2915" value-tone="success" trend="在线率 75.9% · ↑ 12" trend-tone="neutral" :spark="[28,26,30,34,32,38,40,42,44]" />
      <stat-card label="录像占用" value="187 TB" value-tone="warning" trend="存储 78% · 预计 13 天后触顶" trend-tone="neutral">
        <template #extra>
          <div class="kpi-progress">
            <div class="kpi-progress__fill kpi-progress__fill--gradient-warm" style="width:78%"></div>
          </div>
        </template>
      </stat-card>
      <stat-card label="待处理告警" :value="23" value-tone="error" trend="严重 3 · 紧急 7 · 一般 13" trend-tone="neutral">
        <template #extra>
          <div class="kpi-dots">
            <span class="gb-dot gb-dot--error" /><span class="gb-dot gb-dot--error" /><span class="gb-dot gb-dot--error" />
            <span class="gb-dot gb-dot--warning" /><span class="gb-dot gb-dot--warning" /><span class="gb-dot gb-dot--warning" /><span class="gb-dot gb-dot--warning" />
            <span class="gb-dot gb-dot--info" /><span class="gb-dot gb-dot--info" /><span class="gb-dot gb-dot--info" /><span class="gb-dot gb-dot--info" />
          </div>
        </template>
      </stat-card>
    </section>

    <!-- 网络流量 + 通道在线分布 -->
    <section class="gb-grid gb-grid--2col">
      <article class="gb-card">
        <header class="gb-card-title"><span>网络流量 · 上下行</span><span class="meta">最近 24 小时 · Mbps · 每小时</span></header>
        <svg viewBox="0 0 600 200" width="100%" height="200" preserveAspectRatio="none">
          <g stroke="var(--border-subtle)" stroke-dasharray="2,3">
            <line x1="0" y1="40" x2="600" y2="40" />
            <line x1="0" y1="80" x2="600" y2="80" />
            <line x1="0" y1="120" x2="600" y2="120" />
            <line x1="0" y1="160" x2="600" y2="160" />
          </g>
          <path d="M0,150 C40,140 80,100 120,90 C160,80 200,110 240,100 C280,90 320,60 360,70 C400,80 440,50 480,40 C520,30 560,20 600,30 L600,200 L0,200 Z" fill="var(--brand-primary-100)" />
          <path d="M0,150 C40,140 80,100 120,90 C160,80 200,110 240,100 C280,90 320,60 360,70 C400,80 440,50 480,40 C520,30 560,20 600,30" fill="none" stroke="var(--brand-primary-300)" stroke-width="1.6" />
          <path d="M0,170 C40,160 80,140 120,130 C160,120 200,150 240,140 C280,130 320,100 360,110 C400,120 440,90 480,80 C520,70 560,60 600,70 L600,200 L0,200 Z" fill="var(--brand-primary-200)" opacity="0.4" />
          <path d="M0,170 C40,160 80,140 120,130 C160,120 200,150 240,140 C280,130 320,100 360,110 C400,120 440,90 480,80 C520,70 560,60 600,70" fill="none" stroke="var(--brand-primary-500)" stroke-width="1.6" />
          <g font-family="var(--font-mono)" font-size="10" fill="var(--text-tertiary)">
            <text x="0" y="195">00:00</text><text x="100" y="195">04:00</text><text x="200" y="195">08:00</text>
            <text x="300" y="195">12:00</text><text x="400" y="195">16:00</text><text x="500" y="195">20:00</text>
          </g>
        </svg>
        <div class="chart-legend">
          <span><i class="legend-swatch" style="background:var(--brand-primary-300)"></i> 上行 24.6 Gbps</span>
          <span><i class="legend-swatch" style="background:var(--brand-primary-500)"></i> 下行 38.2 Gbps</span>
          <span class="text-tertiary">峰值 16:18</span>
        </div>
      </article>

      <article class="gb-card">
        <header class="gb-card-title"><span>通道在线分布</span><span class="meta">实时</span></header>
        <div class="donut">
          <svg viewBox="0 0 120 120" width="140" height="140">
            <circle cx="60" cy="60" r="50" stroke="var(--bg-elevated)" stroke-width="14" fill="none" />
            <circle cx="60" cy="60" r="50" stroke="var(--state-success)" stroke-width="14" fill="none" stroke-dasharray="238 314" transform="rotate(-90 60 60)" stroke-linecap="round" />
            <circle cx="60" cy="60" r="50" stroke="var(--state-warning)" stroke-width="14" fill="none" stroke-dasharray="32 314" stroke-dashoffset="-238" transform="rotate(-90 60 60)" stroke-linecap="round" />
            <circle cx="60" cy="60" r="50" stroke="var(--state-error)" stroke-width="14" fill="none" stroke-dasharray="14 314" stroke-dashoffset="-270" transform="rotate(-90 60 60)" stroke-linecap="round" />
            <text x="60" y="58" text-anchor="middle" font-family="var(--font-mono)" font-size="20" font-weight="700" fill="var(--text-primary)">75.9%</text>
            <text x="60" y="74" text-anchor="middle" font-size="10" fill="var(--text-tertiary)">在线率</text>
          </svg>
          <ul class="donut-list">
            <li><span class="gb-dot gb-dot--success" /> 在线 <span class="mono ml-a">2,915</span></li>
            <li><span class="gb-dot gb-dot--warning" /> 离线 <span class="mono ml-a">926</span></li>
            <li><span class="gb-dot gb-dot--error" /> 故障 <span class="mono ml-a">23</span></li>
          </ul>
        </div>
      </article>
    </section>

    <!-- 重点通道 + 节点负载 -->
    <section class="gb-grid gb-grid--2col">
      <article class="gb-card">
        <header class="gb-card-title">
          <span>重点通道 · 实时预览</span>
          <a class="meta text-primary-accent" href="javascript:;" @click="$router.push('/live')">查看完整视频墙 →</a>
        </header>
        <div class="grid-video">
          <video-cell v-for="(v, i) in featured" :key="i" :title="v.title" :no="i + 1" :state="v.state" />
        </div>
      </article>

      <article class="gb-card">
        <header class="gb-card-title"><span>节点负载 Top 6</span><span class="meta">CPU · 5 分钟平均</span></header>
        <ul class="load-list">
          <li v-for="n in nodes" :key="n.name">
            <div class="load-row">
              <span class="mono text-xs">{{ n.name }}</span>
              <span class="mono text-xs" :class="n.tone">{{ n.value }}%</span>
            </div>
            <div class="gb-progress">
              <div class="gb-progress__fill" :class="`gb-progress__fill--${n.tone}`" :style="{ width: n.value + '%' }" />
            </div>
          </li>
        </ul>
      </article>
    </section>

    <!-- 告警 -->
    <article class="gb-card">
      <header class="gb-card-title">
        <span>最新告警</span>
        <div class="gb-toolbar">
          <button class="gb-tab is-active">全部</button>
          <button class="gb-tab">严重</button>
          <button class="gb-tab">离线</button>
          <button class="gb-tab">录像丢失</button>
          <a class="meta text-primary-accent" href="javascript:;" @click="$router.push('/alarm')">查看全部告警 →</a>
        </div>
      </header>
      <el-table :data="alarms" stripe size="small" style="width: 100%">
        <el-table-column prop="time" label="时间" width="120">
          <template slot-scope="{ row }"><span class="mono text-tertiary">{{ row.time }}</span></template>
        </el-table-column>
        <el-table-column prop="level" label="级别" width="100">
          <template slot-scope="{ row }"><span :class="['gb-chip', 'gb-chip--' + row.levelTone]">{{ row.level }}</span></template>
        </el-table-column>
        <el-table-column prop="target" label="通道 / 设备" min-width="220" />
        <el-table-column prop="event" label="事件" min-width="220" />
        <el-table-column prop="location" label="位置" min-width="160">
          <template slot-scope="{ row }"><span class="text-tertiary">{{ row.location }}</span></template>
        </el-table-column>
        <el-table-column prop="status" label="状态" width="120">
          <template slot-scope="{ row }"><span :class="['gb-chip', 'gb-chip--' + row.statusTone]">{{ row.status }}</span></template>
        </el-table-column>
        <el-table-column label="操作" width="100" align="right">
          <template slot-scope="{ row }"><button class="gb-btn-link">处理</button></template>
        </el-table-column>
      </el-table>
    </article>
  </div>
</template>

<script>
import StatCard from '@/components/StatCard'
import VideoCell from '@/components/VideoCell'

export default {
  name: 'Console',
  components: { StatCard, VideoCell },
  data() {
    return {
      refreshSec: 2,
      range: '今日',
      ranges: ['今日', '7 日', '30 日', '自定义'],
      nowText: '2026-07-05 · 周日 · 16:42 (Asia/Shanghai)',
      featured: [
        { title: '海珠门岗 · 东', state: 'live' },
        { title: '高速 K127', state: 'live' },
        { title: '天河城 4F', state: 'live' },
        { title: '停车场 B2', state: 'live' },
        { title: '黄埔仓库', state: 'rec' },
        { title: '白云机场', state: 'live' },
        { title: '番禺园区', state: 'offline' },
        { title: '番禺大桥', state: 'live' }
      ],
      nodes: [
        { name: 'zlm-edge-01', value: 38, tone: 'success' },
        { name: 'zlm-edge-02', value: 52, tone: 'success' },
        { name: 'zlm-core-01', value: 71, tone: 'warning' },
        { name: 'sip-gw-shanghai', value: 23, tone: 'success' },
        { name: 'sip-gw-beijing', value: 92, tone: 'error' },
        { name: 'jt-tcp-808', value: 46, tone: 'success' }
      ],
      alarms: [
        { time: '16:42:18', level: '严重', levelTone: 'error', target: '41042200001320000102', event: '视频信号丢失', location: '海珠门岗 · 东', status: '未处理', statusTone: 'error' },
        { time: '16:41:50', level: '紧急', levelTone: 'warning', target: 'sip-gw-beijing', event: 'SIP 注册失败 · 超时', location: '北京节点', status: '处理中', statusTone: 'warning' },
        { time: '16:38:09', level: '一般', levelTone: 'info', target: '44010000001310000001', event: '录像计划触发 · 24×7', location: '天河城 4F', status: '已闭环', statusTone: 'success' },
        { time: '16:32:44', level: '紧急', levelTone: 'warning', target: 'JT-粤B·A8888', event: 'GPS 异常 · 上次定位 8 分钟前', location: '天河城 4F', status: '未处理', statusTone: 'error' },
        { time: '16:28:21', level: '一般', levelTone: 'info', target: 'zlm-edge-04', event: '存储节点切换', location: '边缘节点', status: '已闭环', statusTone: 'success' }
      ]
    }
  }
}
</script>

<style lang="scss" scoped>
.console-page { gap: 14px; }
.kpi-progress { width: 100%; height: 6px; background: var(--bg-elevated); border-radius: 999px; margin-top: 6px; overflow: hidden; }
.kpi-progress__fill { height: 100%; border-radius: 999px; }
.kpi-progress__fill--gradient-warm { background: linear-gradient(90deg, var(--state-warning), var(--state-error)); }
.kpi-dots { display: flex; gap: 4px; margin-top: 6px; }
.chart-legend { display: flex; gap: 18px; margin-top: 8px; font-size: 11px; color: var(--text-tertiary); }
.legend-swatch { display: inline-block; width: 10px; height: 10px; border-radius: 2px; margin-right: 6px; vertical-align: middle; }
.donut { display: flex; flex-direction: column; align-items: center; gap: 8px; }
.donut-list { list-style: none; padding: 0; margin: 8px 0 0; width: 100%; font-size: 11px; color: var(--text-secondary); li { display: flex; align-items: center; gap: 8px; padding: 4px 0; } .ml-a { margin-left: auto; } }
.grid-video { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; }
@media (max-width: 768px) { .grid-video { grid-template-columns: repeat(2, 1fr); } }
.load-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 12px; }
.load-row { display: flex; justify-content: space-between; margin-bottom: 4px; }
.text-success { color: var(--state-success); }
.text-warning { color: var(--state-warning); }
.text-error { color: var(--state-error); }
</style>
