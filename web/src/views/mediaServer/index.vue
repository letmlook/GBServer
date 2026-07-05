<template>
  <div class="gb-page">
    <div class="gb-page__header">
      <div>
        <h1 class="gb-page__title">媒体服务器 · ZLMediaKit</h1>
        <p class="gb-page__subtitle">14 个 ZLM 节点 · 边缘 10 · 核心 4 · 总带宽 168 Gbps</p>
      </div>
      <div class="gb-page__actions">
        <button class="gb-btn">健康检查</button>
        <button class="gb-btn">导入</button>
        <button class="gb-btn gb-btn--primary">+ 新增节点</button>
      </div>
    </div>

    <section class="gb-grid gb-grid--3col">
      <article v-for="n in nodes" :key="n.name" class="node-card gb-card" :class="`tone-${n.tone}`">
        <header class="node-card__head">
          <div>
            <div class="node-card__name">{{ n.name }}</div>
            <div class="node-card__ip mono">{{ n.ip }}</div>
          </div>
          <span :class="['gb-chip', 'gb-chip--' + (n.tone === 'error' ? 'error' : n.tone === 'warning' ? 'warning' : 'success')]">
            <span :class="['gb-dot', 'gb-dot--' + (n.tone === 'error' ? 'error' : n.tone === 'warning' ? 'warning' : 'success')]" />
            {{ n.stateLabel }}
          </span>
        </header>
        <ul class="node-card__meta">
          <li><span>CPU</span><span class="mono">{{ n.cpu }}%</span><div class="gb-progress"><div class="gb-progress__fill" :class="`gb-progress__fill--${n.tone === 'error' ? 'error' : n.tone === 'warning' ? 'warning' : 'success'}`" :style="{ width: n.cpu + '%' }" /></div></li>
          <li><span>内存</span><span class="mono">{{ n.mem }}%</span><div class="gb-progress"><div class="gb-progress__fill" :class="`gb-progress__fill--${n.tone === 'error' ? 'error' : n.tone === 'warning' ? 'warning' : 'success'}`" :style="{ width: n.mem + '%' }" /></div></li>
          <li><span>带宽</span><span class="mono">{{ n.bandwidth }} / 10 Gbps</span><div class="gb-progress"><div class="gb-progress__fill" :class="`gb-progress__fill--${n.tone === 'error' ? 'error' : n.tone === 'warning' ? 'warning' : 'success'}`" :style="{ width: n.bandwidthPct + '%' }" /></div></li>
          <li><span>推流数</span><span class="mono">{{ n.streams }}</span></li>
          <li><span>运行时长</span><span class="mono">{{ n.uptime }}</span></li>
        </ul>
        <footer class="node-card__foot">
          <span class="text-tertiary text-xs">v{{ n.version }}</span>
          <button class="gb-btn-link">详情</button>
        </footer>
      </article>
    </section>

    <article class="gb-card">
      <header class="gb-card-title"><span>节点表格视图</span><span class="meta">支持全局排序 / 筛选</span></header>
      <el-table :data="nodes" stripe size="small" style="width:100%">
        <el-table-column prop="name" label="节点名" min-width="180" />
        <el-table-column prop="ip" label="地址" min-width="160">
          <template slot-scope="{ row }"><span class="mono">{{ row.ip }}</span></template>
        </el-table-column>
        <el-table-column prop="type" label="类型" width="100" />
        <el-table-column prop="cpu" label="CPU" width="100">
          <template slot-scope="{ row }"><span class="mono">{{ row.cpu }}%</span></template>
        </el-table-column>
        <el-table-column prop="mem" label="内存" width="100">
          <template slot-scope="{ row }"><span class="mono">{{ row.mem }}%</span></template>
        </el-table-column>
        <el-table-column prop="streams" label="推流数" width="100" />
        <el-table-column prop="stateLabel" label="状态" width="100">
          <template slot-scope="{ row }"><span :class="['gb-chip', 'gb-chip--' + (row.tone === 'error' ? 'error' : row.tone === 'warning' ? 'warning' : 'success')]">{{ row.stateLabel }}</span></template>
        </el-table-column>
        <el-table-column prop="uptime" label="运行时长" min-width="120" />
        <el-table-column label="操作" align="right" width="180">
          <template slot-scope="{ row }">
            <button class="gb-btn-link">重启</button>
            <button class="gb-btn-link">配置</button>
            <button class="gb-btn-link">日志</button>
          </template>
        </el-table-column>
      </el-table>
    </article>
  </div>
</template>

<script>
export default {
  name: 'MediaServer',
  data() {
    return {
      nodes: [
        { name: 'zlm-edge-01', ip: '10.21.4.21', type: '边缘', cpu: 38, mem: 52, bandwidth: '3.2', bandwidthPct: 32, streams: 156, uptime: '14 天', version: '8.4', tone: 'success', stateLabel: '在线' },
        { name: 'zlm-edge-02', ip: '10.21.4.22', type: '边缘', cpu: 52, mem: 61, bandwidth: '4.4', bandwidthPct: 44, streams: 192, uptime: '14 天', version: '8.4', tone: 'success', stateLabel: '在线' },
        { name: 'zlm-edge-03', ip: '10.21.4.23', type: '边缘', cpu: 46, mem: 58, bandwidth: '3.8', bandwidthPct: 38, streams: 178, uptime: '14 天', version: '8.4', tone: 'success', stateLabel: '在线' },
        { name: 'zlm-edge-04', ip: '10.21.4.24', type: '边缘', cpu: 71, mem: 78, bandwidth: '7.1', bandwidthPct: 71, streams: 218, uptime: '6 天', version: '8.4', tone: 'warning', stateLabel: '高负载' },
        { name: 'zlm-core-01', ip: '10.20.4.11', type: '核心', cpu: 38, mem: 52, bandwidth: '3.2', bandwidthPct: 32, streams: 156, uptime: '30 天', version: '8.5', tone: 'success', stateLabel: '在线' },
        { name: 'zlm-core-02', ip: '10.20.4.12', type: '核心', cpu: 92, mem: 88, bandwidth: '9.6', bandwidthPct: 96, streams: 412, uptime: '30 天', version: '8.5', tone: 'error', stateLabel: '告警' }
      ]
    }
  }
}
</script>

<style lang="scss" scoped>
.gb-grid--3col { display: grid; grid-template-columns: repeat(3, 1fr); gap: 14px; }
@media (max-width: 1280px) { .gb-grid--3col { grid-template-columns: repeat(2, 1fr); } }
@media (max-width: 768px) { .gb-grid--3col { grid-template-columns: 1fr; } }

.node-card { display: flex; flex-direction: column; gap: 12px; }
.node-card.tone-warning { border-color: rgba(234,138,12,.40); }
.node-card.tone-error   { border-color: rgba(220,38,38,.40); }
.node-card__head { display: flex; justify-content: space-between; align-items: flex-start; }
.node-card__name { font-size: var(--text-md); font-weight: 600; }
.node-card__ip   { font-size: 11px; color: var(--text-tertiary); margin-top: 2px; }
.node-card__meta { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 8px; }
.node-card__meta li { display: grid; grid-template-columns: 64px 1fr 110px; align-items: center; gap: 8px; font-size: 11px; color: var(--text-secondary); }
.node-card__meta li span:nth-child(2) { text-align: right; }
.node-card__meta li .gb-progress { grid-column: 1 / 4; }
.node-card__foot { display: flex; justify-content: space-between; align-items: center; padding-top: 6px; border-top: 1px solid var(--border-subtle); }
</style>
