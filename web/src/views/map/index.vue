<template>
  <div class="map-page gb-page">
    <div class="gb-page__header">
      <div>
        <h1 class="gb-page__title">GIS 地图 · 设备地理分布</h1>
        <p class="gb-page__subtitle">共 3,841 个通道 · 在线 2,915 · 离线 926 · 点击地图可定位到具体设备</p>
      </div>
      <div class="gb-toolbar">
        <button class="gb-tab" :class="{ 'is-active': layer === 'all' }" @click="layer='all'">全部</button>
        <button class="gb-tab" :class="{ 'is-active': layer === 'live' }" @click="layer='live'">仅在线</button>
        <button class="gb-tab" :class="{ 'is-active': layer === 'alert' }" @click="layer='alert'">告警</button>
        <button class="gb-tab" :class="{ 'is-active': layer === 'jt' }" @click="layer='jt'">车载</button>
        <button class="gb-btn gb-btn--primary">+ 添加设备</button>
      </div>
    </div>

    <section class="map-shell gb-card" style="padding:0">
      <!-- 地图区 -->
      <div class="map-area">
        <svg viewBox="0 0 1200 600" class="map-canvas" preserveAspectRatio="xMidYMid slice">
          <!-- 简化地图底图 -->
          <rect x="0" y="0" width="1200" height="600" fill="#eaf1f8" />
          <path d="M0,300 C200,200 400,400 600,300 C800,200 1000,400 1200,300 L1200,600 L0,600 Z" fill="#dceaf3" />
          <path d="M0,300 C200,200 400,400 600,300 C800,200 1000,400 1200,300" stroke="#9ad0e6" stroke-width="2" fill="none" />
          <path d="M200,0 C300,150 250,300 350,450 C400,520 500,580 600,600" stroke="#5eb4d4" stroke-width="3" fill="none" opacity="0.6" />
          <path d="M800,0 C700,150 750,300 650,450 C600,520 500,580 400,600" stroke="#5eb4d4" stroke-width="3" fill="none" opacity="0.6" />
          <g font-family="var(--font-sans)" font-size="11" fill="#7e8ea3">
            <text x="200" y="120">天河区</text>
            <text x="800" y="120">海珠区</text>
            <text x="100" y="380">番禺区</text>
            <text x="950" y="430">黄埔区</text>
            <text x="500" y="540">南沙区</text>
          </g>
          <!-- 设备点位 -->
          <g v-for="(d, i) in points" :key="i">
            <circle v-if="!d.alert" :cx="d.x" :cy="d.y" :r="d.size || 5" :fill="d.tone" opacity="0.85" />
            <circle v-if="!d.alert" :cx="d.x" :cy="d.y" r="10" :stroke="d.tone" stroke-width="1.5" fill="none" opacity="0.5">
              <animate attributeName="r" from="6" to="14" dur="2.4s" repeatCount="indefinite" />
              <animate attributeName="opacity" from="0.5" to="0" dur="2.4s" repeatCount="indefinite" />
            </circle>
            <g v-else>
              <circle :cx="d.x" :cy="d.y" r="9" :fill="d.tone" />
              <circle :cx="d.x" :cy="d.y" r="14" :stroke="d.tone" stroke-width="2" fill="none" opacity="0.6">
                <animate attributeName="r" from="10" to="22" dur="1.6s" repeatCount="indefinite" />
                <animate attributeName="opacity" from="0.6" to="0" dur="1.6s" repeatCount="indefinite" />
              </circle>
            </g>
          </g>
          <!-- 选中设备 -->
          <g v-if="selected">
            <circle :cx="selected.x" :cy="selected.y" r="12" fill="none" stroke="var(--brand-primary-500)" stroke-width="2" />
            <rect :x="selected.x + 14" :y="selected.y - 30" width="160" height="50" rx="6" fill="white" stroke="var(--brand-primary-300)" />
            <text :x="selected.x + 22" :y="selected.y - 14" font-size="11" font-weight="600" fill="var(--text-primary)">{{ selected.name }}</text>
            <text :x="selected.x + 22" :y="selected.y" font-size="10" fill="var(--text-tertiary)">{{ selected.id }} · {{ selected.state }}</text>
          </g>
        </svg>

        <!-- 地图工具 -->
        <div class="map-toolbar">
          <button class="map-tool">＋</button>
          <button class="map-tool">－</button>
          <button class="map-tool">⌖</button>
          <button class="map-tool">⊕</button>
        </div>
        <div class="map-legend">
          <span><i class="legend-dot" style="background:var(--state-success)" /> 在线</span>
          <span><i class="legend-dot" style="background:var(--state-warning)" /> 离线</span>
          <span><i class="legend-dot" style="background:var(--state-error)" /> 告警</span>
          <span><i class="legend-dot" style="background:var(--brand-primary-500)" /> 选中</span>
        </div>
        <div class="map-scale">200m / scale</div>
      </div>

      <!-- 右侧设备列表 -->
      <aside class="map-list">
        <div class="map-list__head">
          <div class="gb-search" style="flex:1">
            <svg viewBox="0 0 24 24" fill="none"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.6"/><path d="M21 21l-4.35-4.35" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
            <input v-model="kw" placeholder="搜索设备…">
          </div>
        </div>
        <ul class="map-list__rows">
          <li v-for="d in listFiltered" :key="d.id" :class="{ 'is-active': selected && selected.id === d.id }" @click="selected = d">
            <div class="row-1">
              <span :class="['gb-dot', 'gb-dot--' + d.dotTone]" />
              <span class="row-name">{{ d.name }}</span>
              <span v-if="d.alert" class="gb-chip gb-chip--error" style="font-size:9px">ALERT</span>
            </div>
            <div class="row-2 mono text-tertiary">{{ d.id }} · {{ d.distance }}</div>
          </li>
        </ul>
      </aside>
    </section>
  </div>
</template>

<script>
export default {
  name: 'GisMap',
  data() {
    return {
      layer: 'all',
      kw: '',
      selected: null,
      points: [],
      list: [
        { id: '41042200001320000102', name: '海珠门岗 · 东', x: 760, y: 220, tone: 'var(--state-error)', dotTone: 'error', size: 8, alert: true, distance: '1.2km', state: '告警' },
        { id: '41042200001320000105', name: '海珠仓库 · 北', x: 800, y: 320, tone: 'var(--state-success)', dotTone: 'success', state: '在线' },
        { id: '44010000001310000001', name: '天河城 4F', x: 380, y: 200, tone: 'var(--state-success)', dotTone: 'success', state: '在线' },
        { id: '44010000001310000002', name: '天河城 5F', x: 360, y: 240, tone: 'var(--state-success)', dotTone: 'success', state: '在线' },
        { id: '44010000001310000004', name: '天河区政府', x: 420, y: 280, tone: 'var(--state-success)', dotTone: 'success', state: '在线' },
        { id: '44010000001310000007', name: '天河城 3F 故障', x: 390, y: 180, tone: 'var(--state-warning)', dotTone: 'warning', state: '离线' },
        { id: 'JT-粤B·A8888', name: '公交 86 路 · A8888', x: 510, y: 360, tone: 'var(--state-error)', dotTone: 'error', alert: true, size: 6, state: 'GPS 异常' },
        { id: '51060000001310000001', name: '番禺园区', x: 180, y: 380, tone: 'var(--state-success)', dotTone: 'success', state: '在线' },
        { id: '51060000001310000002', name: '番禺大桥', x: 250, y: 450, tone: 'var(--state-success)', dotTone: 'success', state: '在线' },
        { id: '51010000001310000001', name: '黄埔仓库', x: 920, y: 410, tone: 'var(--state-success)', dotTone: 'success', state: '在线' },
        { id: '51010000001310000002', name: '黄埔园区', x: 950, y: 440, tone: 'var(--state-success)', dotTone: 'success', state: '在线' }
      ]
    }
  },
  computed: {
    listFiltered() {
      const kw = this.kw.toLowerCase()
      return this.list.filter(d => !kw || d.name.toLowerCase().includes(kw) || d.id.toLowerCase().includes(kw))
    }
  },
  mounted() {
    this.points = this.list.map(d => ({ x: d.x, y: d.y, tone: d.tone, alert: d.alert, size: d.size }))
    this.selected = this.list[0]
  }
}
</script>

<style lang="scss" scoped>
.map-page { gap: 12px; padding-top: 16px; }
.map-shell { display: grid; grid-template-columns: 1fr 320px; height: calc(100vh - 56px - 38px - 92px); overflow: hidden; }
@media (max-width: 1024px) { .map-shell { grid-template-columns: 1fr; height: auto; } }
.map-area { position: relative; }
.map-canvas { width: 100%; height: 100%; }
.map-toolbar { position: absolute; right: 12px; top: 12px; display: flex; flex-direction: column; gap: 4px; }
.map-tool { width: 32px; height: 32px; display: grid; place-items: center; background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 4px; cursor: pointer; color: var(--text-secondary); }
.map-tool:hover { color: var(--brand-primary-500); border-color: var(--brand-primary-300); }
.map-legend { position: absolute; left: 12px; top: 12px; display: flex; gap: 10px; padding: 6px 10px; background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 4px; font-size: 11px; }
.legend-dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; margin-right: 4px; }
.map-scale { position: absolute; right: 12px; bottom: 12px; padding: 4px 8px; background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 4px; font-size: 10px; color: var(--text-tertiary); }

.map-list { background: var(--bg-surface); border-left: 1px solid var(--border-subtle); display: flex; flex-direction: column; }
.map-list__head { padding: 12px; border-bottom: 1px solid var(--border-subtle); }
.map-list__rows { list-style: none; margin: 0; padding: 0; overflow-y: auto; flex: 1; }
.map-list__rows li { padding: 10px 12px; border-bottom: 1px solid var(--border-subtle); cursor: pointer; }
.map-list__rows li:hover { background: var(--bg-hover); }
.map-list__rows li.is-active { background: rgba(11,138,178,.10); }
.row-1 { display: flex; align-items: center; gap: 6px; font-size: 12px; }
.row-name { font-weight: 500; }
.row-2 { font-size: 10px; margin-top: 2px; }
</style>
