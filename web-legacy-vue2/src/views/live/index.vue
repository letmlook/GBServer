<template>
  <div class="live-wall">
    <!-- 左侧设备树 -->
    <aside class="live-tree">
      <div class="live-tree__head">
        <span class="text-sm font-semibold">设备分组</span>
        <button class="gb-btn-link" @click="expandAll = !expandAll">{{ expandAll ? '收起全部' : '展开全部' }}</button>
      </div>
      <input v-model="treeFilter" class="live-tree__search" placeholder="筛选通道…">
      <div class="live-tree__list">
        <div v-for="g in groups" :key="g.name" class="tree-node" :class="{ 'is-open': g.open }">
          <div class="tree-node__row" @click="g.open = !g.open">
            <span class="caret">{{ g.open ? '▾' : '▸' }}</span>
            <span class="emoji">{{ g.emoji }}</span>
            <span class="flex-1">{{ g.name }}</span>
            <span class="text-tertiary text-xs">{{ g.total }}</span>
          </div>
          <div v-if="g.open" class="tree-children">
            <div v-for="c in g.children" :key="c.name" class="tree-leaf" :class="{ 'is-active': activeLeaf === c.name }" @click="activeLeaf = c.name">
              <span :class="['gb-dot', 'gb-dot--' + (c.state || 'success')]" />
              <span class="flex-1">{{ c.name }}</span>
              <span class="text-tertiary text-xs">{{ c.count }}</span>
            </div>
          </div>
        </div>
      </div>
    </aside>

    <!-- 中间视频墙 -->
    <section class="live-main">
      <div class="filter-bar">
        <span v-for="f in filters" :key="f.label" class="gb-pill" :class="{ 'is-active': activeFilter === f.label }" @click="activeFilter = f.label">
          {{ f.label }} {{ f.count.toLocaleString() }}
        </span>
        <div class="flex-1" />
        <div class="layout-toolbar">
          <button v-for="l in layouts" :key="l" class="layout-btn" :class="{ 'is-active': layout === l }" @click="layout = l">{{ l }}</button>
        </div>
      </div>

      <div class="stat-strip">
        <span class="stat-pill" style="background:rgba(22,163,74,.10);color:var(--state-success)"><span class="gb-dot gb-dot--success" /> 16 路在线</span>
        <span class="stat-pill" style="background:rgba(11,138,178,.10);color:var(--brand-primary-500)">总码率 38.4 Mbps</span>
        <span class="stat-pill" style="background:rgba(234,138,12,.10);color:var(--state-warning)">丢包率 0.02%</span>
        <span class="text-tertiary text-xs">协议混合: H.264 ×14 · H.265 ×2</span>
        <div class="flex-1" />
        <button class="gb-btn gb-btn--danger"><span class="gb-dot gb-dot--error" /> 紧急录像</button>
        <button class="gb-btn gb-btn--primary">全屏轮播</button>
      </div>

      <div :class="['wall-grid', `wall-grid--${layout}`]">
        <video-cell v-for="(c, i) in cells" :key="i" :title="c.title" :no="String(i + 1).padStart(2, '0')" :state="c.state" />
      </div>
    </section>

    <!-- 右侧详情 -->
    <aside class="live-detail">
      <div class="video-cell gb-video-cell" style="aspect-ratio:16/9; border-color:var(--brand-primary-500)">
        <img src="/static/images/bg19.webp" alt="">
        <div class="gb-video-cell__overlay">
          <div class="gb-video-cell__overlay-top">
            <span class="gb-chip gb-chip--error" style="background:rgba(239,68,68,.85)">● LIVE</span>
            <span class="mono" style="font-size:11px">41042200001320000102</span>
          </div>
          <div class="gb-video-cell__overlay-bottom">
            <div>
              <div>海珠门岗 · 东 · 4K</div>
              <div style="font-size:10px; opacity:0.6">25 fps · 6 Mbps · H.265</div>
            </div>
            <div class="mono" style="font-size:10px; opacity:0.8">16:42:18</div>
          </div>
        </div>
      </div>

      <article class="gb-card">
        <div class="gb-card-title" style="margin-bottom:10px"><span>通道详情</span></div>
        <table class="kv">
          <tr><td class="k">国标 ID</td><td class="v mono">41042200001320000102</td></tr>
          <tr><td class="k">父设备</td><td class="v mono">44010000001310000001</td></tr>
          <tr><td class="k">设备 IP</td><td class="v mono">10.21.4.118</td></tr>
          <tr><td class="k">经纬度</td><td class="v">113.27°N · 23.13°E</td></tr>
          <tr><td class="k">在线时长</td><td class="v">14 天 6 时</td></tr>
          <tr><td class="k">所属平台</td><td class="v">天河中心</td></tr>
          <tr><td class="k">视频参数</td><td class="v">4K · H.265 · 25 fps</td></tr>
        </table>
      </article>

      <article class="gb-card">
        <div class="gb-card-title"><span>PTZ 控制</span><span class="meta">云台</span></div>
        <div class="ptz-pad">
          <button class="ptz-btn">↑</button>
          <div class="ptz-row">
            <button class="ptz-btn">←</button>
            <button class="ptz-btn primary">●</button>
            <button class="ptz-btn">→</button>
          </div>
          <button class="ptz-btn">↓</button>
        </div>
        <div class="ptz-zoom">
          <button class="gb-btn">−</button>
          <span class="text-tertiary">变倍</span>
          <button class="gb-btn">+</button>
          <span class="text-tertiary" style="margin-left:12px">聚焦</span>
          <button class="gb-btn">+</button>
        </div>
      </article>
    </aside>
  </div>
</template>

<script>
import VideoCell from '@/components/VideoCell'

export default {
  name: 'LiveWall',
  components: { VideoCell },
  data() {
    return {
      expandAll: true,
      treeFilter: '',
      activeLeaf: '海珠门岗',
      activeFilter: '全部',
      layout: '4×4',
      filters: [
        { label: '全部', count: 2915 },
        { label: '在线', count: 2890 },
        { label: '录制中', count: 1247 },
        { label: '离线', count: 25 },
        { label: '故障', count: 3 }
      ],
      layouts: ['1×1', '2×2', '4×4', '5×5', '6×6'],
      groups: [
        { name: '海珠区 · 总数 412', emoji: '🏢', total: 412, open: true, children: [
          { name: '海珠门岗', state: 'success', count: 12 },
          { name: '海珠仓库', state: 'success', count: 32 },
          { name: '海珠停车场', state: 'warning', count: 8 },
          { name: '海珠园区 · 故障', state: 'error', count: 3 }
        ] },
        { name: '天河区 · 总数 486', emoji: '🏛️', total: 486, open: true, children: [
          { name: '天河城商圈', state: 'success', count: 98 },
          { name: '天河区政府', state: 'success', count: 12 }
        ] },
        { name: '道路监控 · 总数 1,247', emoji: '🛣️', total: 1247, open: false, children: [
          { name: '高速', state: 'success', count: 623 },
          { name: '主干道', state: 'success', count: 412 },
          { name: '桥梁', state: 'success', count: 212 }
        ] },
        { name: '公交 · 1,025', emoji: '🚌', total: 1025, open: false, children: [] },
        { name: '地铁站 · 671', emoji: '🚇', total: 671, open: false, children: [] }
      ],
      cells: [
        { title: '海珠门岗 · 东', state: 'live' }, { title: '高速 K127', state: 'live' },
        { title: '天河城 4F', state: 'live' }, { title: '停车场 B2', state: 'live' },
        { title: '黄埔仓库', state: 'rec' }, { title: '白云机场', state: 'live' },
        { title: '番禺园区', state: 'offline' }, { title: '番禺大桥', state: 'live' },
        { title: '海珠仓库', state: 'live' }, { title: '天河区政府', state: 'live' },
        { title: '桥梁监测', state: 'live' }, { title: '主干道北', state: 'live' },
        { title: '地铁公园前', state: 'rec' }, { title: '公交 86 路', state: 'live' },
        { title: '园区西门口', state: 'mute' }, { title: '仓库 B 区', state: 'live' }
      ]
    }
  }
}
</script>

<style lang="scss" scoped>
.live-wall {
  display: grid;
  grid-template-columns: 280px 1fr 320px;
  height: calc(100vh - 56px - 38px);
  background: var(--bg-base);
  overflow: hidden;
}
@media (max-width: 1280px) { .live-wall { grid-template-columns: 220px 1fr 280px; } }
@media (max-width: 1024px) { .live-wall { grid-template-columns: 1fr; height: auto; } }

/* 左侧 */
.live-tree { background: var(--bg-surface); border-right: 1px solid var(--border-subtle); padding: 12px; overflow-y: auto; }
.live-tree__head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; }
.live-tree__search { width: 100%; padding: 7px 10px; background: var(--bg-elevated); border: 1px solid var(--border-default); border-radius: 6px; color: var(--text-primary); font-size: 12px; outline: 0; }
.live-tree__list { margin-top: 12px; }
.tree-node__row { display: flex; align-items: center; gap: 6px; padding: 6px 6px; border-radius: 4px; cursor: pointer; font-size: 12px; }
.tree-node__row:hover { background: var(--bg-hover); }
.tree-children { padding-left: 14px; }
.tree-leaf { display: flex; align-items: center; gap: 6px; padding: 4px 6px; border-radius: 4px; cursor: pointer; font-size: 12px; }
.tree-leaf:hover { background: var(--bg-hover); }
.tree-leaf.is-active { background: rgba(11,138,178,.10); color: var(--brand-primary-500); }
.caret { color: var(--text-tertiary); width: 10px; }
.flex-1 { flex: 1; }

/* 中间 */
.live-main { display: flex; flex-direction: column; overflow: hidden; }
.filter-bar { display: flex; align-items: center; gap: 6px; padding: 10px 14px; border-bottom: 1px solid var(--border-subtle); background: var(--bg-surface); flex-wrap: wrap; }
.layout-toolbar { display: flex; gap: 4px; }
.layout-btn { padding: 4px 10px; font-size: 11px; border: 1px solid var(--border-default); background: var(--bg-surface); color: var(--text-secondary); border-radius: 4px; cursor: pointer; }
.layout-btn:hover { border-color: var(--brand-primary-300); color: var(--brand-primary-500); }
.layout-btn.is-active { background: var(--brand-primary-500); color: #fff; border-color: var(--brand-primary-500); }

.stat-strip { display: flex; align-items: center; gap: 10px; padding: 8px 14px; background: var(--bg-surface); border-bottom: 1px solid var(--border-subtle); flex-wrap: wrap; }
.stat-pill { display: inline-flex; align-items: center; gap: 4px; padding: 3px 8px; border-radius: 4px; font-size: 11px; font-weight: 500; }

.wall-grid { flex: 1; padding: 10px; overflow: auto; display: grid; gap: 6px; }
.wall-grid--1x1 { grid-template-columns: 1fr; }
.wall-grid--2x2 { grid-template-columns: repeat(2, 1fr); }
.wall-grid--4x4 { grid-template-columns: repeat(4, 1fr); }
.wall-grid--5x5 { grid-template-columns: repeat(5, 1fr); }
.wall-grid--6x6 { grid-template-columns: repeat(6, 1fr); }

/* 右侧 */
.live-detail { background: var(--bg-surface); border-left: 1px solid var(--border-subtle); padding: 14px; display: flex; flex-direction: column; gap: 14px; overflow-y: auto; }
.kv { width: 100%; font-size: 12px; }
.kv .k { color: var(--text-tertiary); padding: 3px 0; }
.kv .v { text-align: right; }

.ptz-pad { display: flex; flex-direction: column; align-items: center; gap: 4px; margin-bottom: 12px; }
.ptz-row { display: flex; gap: 4px; }
.ptz-btn { width: 32px; height: 32px; border: 1px solid var(--border-default); background: var(--bg-elevated); border-radius: 6px; cursor: pointer; color: var(--text-secondary); }
.ptz-btn:hover { border-color: var(--brand-primary-300); color: var(--brand-primary-500); }
.ptz-btn.primary { background: var(--brand-primary-500); color: #fff; border-color: var(--brand-primary-500); }
.ptz-zoom { display: flex; align-items: center; gap: 6px; font-size: 11px; }
</style>
