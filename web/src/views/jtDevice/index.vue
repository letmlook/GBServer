<template>
  <div class="gb-page">
    <div class="gb-page__header">
      <div>
        <h1 class="gb-page__title">车载终端管理</h1>
        <p class="gb-page__subtitle">JT/T 808/1078 · 1,025 台车载终端 · 公交 482 · 物流 326 · 危险品 217</p>
      </div>
      <div class="gb-page__actions">
        <button class="gb-btn">批量定位</button>
        <button class="gb-btn">导出轨迹</button>
        <button class="gb-btn gb-btn--primary">+ 新增终端</button>
      </div>
    </div>

    <!-- KPI -->
    <section class="gb-grid gb-grid--kpi">
      <stat-card label="终端总数" :value="1025" trend="在线 893" trend-tone="success" :spark="[12,18,20,24,28,32,40,48]" />
      <stat-card label="在线率" value="87.1%" value-tone="success" trend="↑ 1.8% 较昨日" trend-tone="success" :spark="[78,80,82,84,85,86,87,87]" />
      <stat-card label="GPS 信号弱" :value="42" value-tone="warning" trend="11 台超过 5 分钟未上报" trend-tone="neutral" :spark="[10,12,18,20,24,28,32,42]" />
      <stat-card label="告警中" :value="18" value-tone="error" trend="超速 7 · 越界 5 · 其他 6" trend-tone="neutral" :spark="[3,5,6,8,10,12,15,18]" />
    </section>

    <!-- 地图 + 列表 -->
    <section class="jt-shell">
      <article class="gb-card jt-map" style="padding:0">
        <header class="gb-card-title" style="padding:14px 14px 0">
          <span>实时位置</span>
          <span class="meta">8 台选中 · 单击终端聚焦</span>
        </header>
        <div class="jt-map__body">
          <svg viewBox="0 0 800 500" class="map-canvas" preserveAspectRatio="xMidYMid slice">
            <rect x="0" y="0" width="800" height="500" fill="#eaf1f8" />
            <path d="M0,250 C150,180 300,330 450,250 C600,180 700,330 800,250" stroke="#9ad0e6" stroke-width="2" fill="none" />
            <path d="M0,250 C150,180 300,330 450,250 C600,180 700,330 800,250 L800,500 L0,500 Z" fill="#dceaf3" />
            <g v-for="(v, i) in vehicles" :key="i">
              <circle v-if="!v.alert" :cx="v.x" :cy="v.y" r="6" :fill="v.tone" />
              <circle v-if="!v.alert" :cx="v.x" :cy="v.y" r="10" :stroke="v.tone" stroke-width="1.2" fill="none" opacity="0.5">
                <animate attributeName="r" from="6" to="14" dur="2.4s" repeatCount="indefinite" />
                <animate attributeName="opacity" from="0.5" to="0" dur="2.4s" repeatCount="indefinite" />
              </circle>
              <g v-else>
                <rect :x="v.x - 6" :y="v.y - 6" width="12" height="12" rx="2" :fill="v.tone" />
                <circle :cx="v.x" :cy="v.y" r="12" :stroke="v.tone" stroke-width="2" fill="none" opacity="0.6">
                  <animate attributeName="r" from="10" to="22" dur="1.6s" repeatCount="indefinite" />
                  <animate attributeName="opacity" from="0.6" to="0" dur="1.6s" repeatCount="indefinite" />
                </circle>
              </g>
              <text :x="v.x + 8" :y="v.y - 8" font-size="9" fill="var(--text-tertiary)" font-family="var(--font-mono)">{{ v.plate }}</text>
            </g>
            <g v-if="track">
              <path :d="track.path" stroke="var(--brand-primary-500)" stroke-width="2" fill="none" stroke-dasharray="4 4" />
            </g>
          </svg>
        </div>
      </article>

      <article class="gb-card">
        <header class="gb-card-title"><span>终端列表</span>
          <div class="gb-toolbar">
            <input v-model="kw" class="search-mini" placeholder="车牌 / SIM">
            <select v-model="cat" class="search-mini">
              <option value="">全部类别</option>
              <option value="公交">公交</option>
              <option value="物流">物流</option>
              <option value="危险品">危险品</option>
            </select>
          </div>
        </header>
        <ul class="jt-list">
          <li v-for="v in filtered" :key="v.plate" :class="{ 'is-active': track && track.plate === v.plate }" @click="track = v">
            <div class="row-1">
              <span :class="['gb-dot', 'gb-dot--' + v.toneKey]" />
              <span class="row-name mono">{{ v.plate }}</span>
              <span class="text-tertiary text-xs">{{ v.cat }}</span>
              <span v-if="v.alert" class="gb-chip gb-chip--error" style="font-size:9px">ALERT</span>
            </div>
            <div class="row-2 mono text-tertiary">{{ v.sim }} · {{ v.speed }} km/h · {{ v.loc }}</div>
          </li>
        </ul>
      </article>
    </section>

    <!-- 表格 -->
    <article class="gb-card">
      <header class="gb-card-title">
        <span>终端明细</span>
        <div class="gb-toolbar">
          <button class="gb-btn">下发指令</button>
          <button class="gb-btn">参数配置</button>
          <button class="gb-btn gb-btn--danger">注销</button>
        </div>
      </header>
      <el-table :data="tableRows" stripe size="small" style="width:100%">
        <el-table-column type="selection" width="40" />
        <el-table-column prop="plate" label="车牌" min-width="120">
          <template slot-scope="{ row }"><span class="mono">{{ row.plate }}</span></template>
        </el-table-column>
        <el-table-column prop="sim" label="SIM 卡" min-width="160">
          <template slot-scope="{ row }"><span class="mono text-tertiary">{{ row.sim }}</span></template>
        </el-table-column>
        <el-table-column prop="cat" label="类别" width="80" />
        <el-table-column prop="brand" label="设备厂商" min-width="140" />
        <el-table-column prop="state" label="状态" width="100">
          <template slot-scope="{ row }"><span :class="['gb-chip', 'gb-chip--' + row.stateTone]">{{ row.state }}</span></template>
        </el-table-column>
        <el-table-column prop="speed" label="速度" width="80">
          <template slot-scope="{ row }"><span class="mono">{{ row.speed }} km/h</span></template>
        </el-table-column>
        <el-table-column prop="battery" label="电量" width="140">
          <template slot-scope="{ row }">
            <div class="battery" :data-tone="row.battery < 30 ? 'error' : (row.battery < 60 ? 'warning' : 'success')">
              <span class="battery__fill" :style="{ width: row.battery + '%' }" />
              <span class="battery__label mono">{{ row.battery }}%</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="updated" label="最后上报" width="120">
          <template slot-scope="{ row }"><span class="mono text-tertiary">{{ row.updated }}</span></template>
        </el-table-column>
        <el-table-column label="操作" align="right" width="180">
          <template slot-scope="{ row }">
            <button class="gb-btn-link">实时</button>
            <button class="gb-btn-link">回放</button>
            <button class="gb-btn-link">指令</button>
          </template>
        </el-table-column>
      </el-table>
      <el-pagination layout="prev, pager, next, jumper, total" :total="1025" :page-size="20" class="mt-2" />
    </article>
  </div>
</template>

<script>
import StatCard from '@/components/StatCard'

export default {
  name: 'JtDevice',
  components: { StatCard },
  data() {
    return {
      kw: '',
      cat: '',
      track: null,
      vehicles: [
        { plate: '粤B·A8888', sim: '89860416102370001234', cat: '公交', x: 320, y: 220, tone: 'var(--state-error)', toneKey: 'error', speed: 78, loc: '天河城 4F', alert: true },
        { plate: '粤B·B1234', sim: '89860416102370001245', cat: '公交', x: 380, y: 200, tone: 'var(--state-success)', toneKey: 'success', speed: 45, loc: '天河区政府' },
        { plate: '粤A·C5678', sim: '89860416102370001256', cat: '物流', x: 450, y: 280, tone: 'var(--state-success)', toneKey: 'success', speed: 82, loc: '华南快速' },
        { plate: '粤A·D9012', sim: '89860416102370001267', cat: '危险品', x: 280, y: 360, tone: 'var(--state-warning)', toneKey: 'warning', speed: 0, loc: '番禺园区' },
        { plate: '粤B·E3456', sim: '89860416102370001278', cat: '公交', x: 520, y: 320, tone: 'var(--state-success)', toneKey: 'success', speed: 56, loc: '海珠桥' },
        { plate: '粤B·F7890', sim: '89860416102370001289', cat: '物流', x: 600, y: 250, tone: 'var(--state-success)', toneKey: 'success', speed: 64, loc: '黄埔大道' },
        { plate: '粤A·G1121', sim: '89860416102370001290', cat: '公交', x: 180, y: 200, tone: 'var(--state-success)', toneKey: 'success', speed: 38, loc: '越秀公园' },
        { plate: '粤A·H3344', sim: '89860416102370001301', cat: '危险品', x: 680, y: 360, tone: 'var(--state-error)', toneKey: 'error', speed: 92, loc: '南沙港', alert: true }
      ],
      tableRows: [
        { plate: '粤B·A8888', sim: '89860416102370001234', cat: '公交', brand: '锐明视讯', state: '告警', stateTone: 'error', speed: 78, battery: 64, updated: '8 秒前' },
        { plate: '粤B·B1234', sim: '89860416102370001245', cat: '公交', brand: '锐明视讯', state: '在线', stateTone: 'success', speed: 45, battery: 82, updated: '4 秒前' },
        { plate: '粤A·C5678', sim: '89860416102370001256', cat: '物流', brand: '海康车载', state: '在线', stateTone: 'success', speed: 82, battery: 18, updated: '2 秒前' },
        { plate: '粤A·D9012', sim: '89860416102370001267', cat: '危险品', brand: '大华车载', state: '弱信号', stateTone: 'warning', speed: 0, battery: 56, updated: '5 分前' },
        { plate: '粤B·E3456', sim: '89860416102370001278', cat: '公交', brand: '锐明视讯', state: '在线', stateTone: 'success', speed: 56, battery: 91, updated: '3 秒前' },
        { plate: '粤B·F7890', sim: '89860416102370001289', cat: '物流', brand: '海康车载', state: '在线', stateTone: 'success', speed: 64, battery: 72, updated: '6 秒前' },
        { plate: '粤A·G1121', sim: '89860416102370001290', cat: '公交', brand: '海信', state: '在线', stateTone: 'success', speed: 38, battery: 86, updated: '12 秒前' },
        { plate: '粤A·H3344', sim: '89860416102370001301', cat: '危险品', brand: '大华车载', state: '告警', stateTone: 'error', speed: 92, battery: 24, updated: '1 分前' }
      ]
    }
  },
  computed: {
    filtered() {
      return this.vehicles.filter(v => (!this.kw || v.plate.includes(this.kw) || v.sim.includes(this.kw)) && (!this.cat || v.cat === this.cat))
    }
  },
  watch: {
    filtered: {
      immediate: true,
      handler(list) { this.track = { ...list[0], path: `M 100,200 Q 200,150 280,${list[0].y}` } }
    }
  }
}
</script>

<style lang="scss" scoped>
.jt-shell { display: grid; grid-template-columns: 1fr 320px; gap: 14px; }
@media (max-width: 1280px) { .jt-shell { grid-template-columns: 1fr; } }
.jt-map { display: flex; flex-direction: column; }
.jt-map__body { flex: 1; padding: 12px; }
.map-canvas { width: 100%; height: 380px; border-radius: 6px; }

.jt-list { list-style: none; margin: 0; padding: 0; max-height: 380px; overflow-y: auto; }
.jt-list li { padding: 8px 10px; border-bottom: 1px solid var(--border-subtle); cursor: pointer; }
.jt-list li:hover { background: var(--bg-hover); }
.jt-list li.is-active { background: rgba(11,138,178,.10); }
.row-1 { display: flex; align-items: center; gap: 6px; font-size: 12px; }
.row-name { font-weight: 500; }
.row-2 { font-size: 10px; margin-top: 2px; }

.search-mini { padding: 4px 8px; font-size: 11px; border: 1px solid var(--border-default); background: var(--bg-elevated); border-radius: 4px; color: var(--text-primary); outline: 0; }
.search-mini:focus { border-color: var(--brand-primary-300); }

.battery { position: relative; height: 14px; width: 100px; background: var(--bg-elevated); border-radius: 3px; overflow: hidden; }
.battery__fill { display: block; height: 100%; border-radius: 3px; }
.battery[data-tone="success"] .battery__fill { background: var(--state-success); }
.battery[data-tone="warning"] .battery__fill { background: var(--state-warning); }
.battery[data-tone="error"]   .battery__fill { background: var(--state-error); }
.battery__label { position: absolute; inset: 0; display: grid; place-items: center; font-size: 10px; color: var(--text-primary); }
.mt-2 { margin-top: 8px; }
</style>
