<template>
  <div class="playback">
    <aside class="pb-tree">
      <div class="pb-tree__head">
        <span class="text-sm font-semibold">通道目录</span>
        <button class="gb-btn-link">刷新</button>
      </div>
      <div class="gb-search" style="width:100%">
        <svg viewBox="0 0 24 24" fill="none"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.6"/><path d="M21 21l-4.35-4.35" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
        <input v-model="kw" type="text" placeholder="搜索通道…">
      </div>
      <ul class="pb-tree__list">
        <li v-for="n in nodes" :key="n.name" :class="{ 'is-active': active === n.name }" @click="active = n.name">
          <span class="gb-dot gb-dot--success" />
          <span class="flex-1">{{ n.name }}</span>
          <span class="text-tertiary text-xs">{{ n.count }}</span>
        </li>
      </ul>
    </aside>

    <section class="pb-main">
      <div class="gb-filterbar gb-card" style="padding:12px 14px">
        <span class="gb-filterbar__label">日期</span>
        <el-date-picker v-model="date" type="date" size="small" placeholder="选择日期" style="width:140px" />
        <span class="gb-filterbar__label">时间</span>
        <el-time-picker v-model="from" size="small" placeholder="起始" style="width:110px" />
        <span class="text-tertiary">→</span>
        <el-time-picker v-model="to" size="small" placeholder="结束" style="width:110px" />
        <el-checkbox v-model="continuous">连续录像</el-checkbox>
        <div class="gb-filterbar__right">
          <button class="gb-btn"><span class="gb-dot gb-dot--info" /> 切片下载</button>
          <button class="gb-btn gb-btn--primary"><span class="mono">▶</span> 检索</button>
        </div>
      </div>

      <article class="gb-card">
        <header class="gb-card-title">
          <span>海珠门岗 · 东 · 41042200001320000102</span>
          <span class="meta">6 段录像 · 总时长 2 时 14 分 · 1.2 GB</span>
        </header>
        <div class="player">
          <div class="player__viewport">
            <div class="player__overlay-top">
              <span class="gb-chip gb-chip--warning">● REC</span>
              <span class="mono text-xs">2026-07-05 14:22:18</span>
            </div>
            <div class="player__overlay-bottom">
              <span class="mono text-xs">02x</span>
              <span class="mono text-xs">16:42:18</span>
            </div>
            <div class="player__poster">▶</div>
          </div>
          <div class="player__controls">
            <button class="player__btn primary">▶</button>
            <button class="player__btn">⏸</button>
            <button class="player__btn">⏮</button>
            <button class="player__btn">⏭</button>
            <div class="player__progress">
              <div class="player__progress-fill" style="width:32%" />
              <div class="player__progress-marker" style="left:32%" />
            </div>
            <span class="player__time mono">12:18 / 38:42</span>
            <button class="player__btn ghost">⤢</button>
            <button class="player__btn ghost">⤓</button>
            <select class="player__rate">
              <option>1x</option><option>2x</option><option>4x</option><option>8x</option>
            </select>
          </div>
        </div>
      </article>

      <article class="gb-card">
        <header class="gb-card-title">
          <span>时间轴 · 录像段</span>
          <div class="gb-toolbar">
            <button v-for="s in scales" :key="s" class="gb-tab" :class="{ 'is-active': scale === s }" @click="scale = s">{{ s }}</button>
            <span class="text-tertiary text-xs">显示 6 段录像</span>
          </div>
        </header>
        <div class="timeline">
          <div class="timeline__axis">
            <span v-for="h in hours" :key="h" class="mono text-tertiary">{{ h }}</span>
          </div>
          <div class="timeline__row">
            <div v-for="(s, i) in segments" :key="i" class="timeline__seg" :class="`tone-${s.tone}`" :style="s.style" :title="`${s.start} → ${s.end} · ${s.size}`">
              <span class="timeline__seg-label">{{ s.label }}</span>
            </div>
          </div>
        </div>
      </article>

      <article class="gb-card">
        <header class="gb-card-title">
          <span>录像段列表</span>
          <span class="meta">点击条目定位到时间轴</span>
        </header>
        <el-table :data="rows" size="small" stripe style="width:100%">
          <el-table-column type="selection" width="40" />
          <el-table-column prop="no" label="序号" width="60" />
          <el-table-column label="起止时间" min-width="220">
            <template slot-scope="{ row }">
              <span class="mono">{{ row.start }}</span>
              <span class="text-tertiary"> → </span>
              <span class="mono">{{ row.end }}</span>
            </template>
          </el-table-column>
          <el-table-column prop="duration" label="时长" width="100">
            <template slot-scope="{ row }"><span class="mono">{{ row.duration }}</span></template>
          </el-table-column>
          <el-table-column prop="size" label="大小" width="100">
            <template slot-scope="{ row }"><span class="mono">{{ row.size }}</span></template>
          </el-table-column>
          <el-table-column prop="type" label="类型" width="100">
            <template slot-scope="{ row }"><span :class="['gb-chip', 'gb-chip--' + row.tone]">{{ row.type }}</span></template>
          </el-table-column>
          <el-table-column prop="trigger" label="触发" />
          <el-table-column label="操作" align="right" width="200">
            <template slot-scope="{ row }">
              <button class="gb-btn-link">下载</button>
              <button class="gb-btn-link">播放</button>
              <button class="gb-btn-link">共享</button>
            </template>
          </el-table-column>
        </el-table>
      </article>
    </section>
  </div>
</template>

<script>
export default {
  name: 'Playback',
  data() {
    return {
      kw: '',
      active: '海珠门岗 · 东',
      date: new Date(),
      from: '08:00:00', to: '20:00:00',
      continuous: true,
      scale: '15 分',
      scales: ['5 分', '15 分', '1 时', '6 时', '24 时'],
      nodes: [
        { name: '海珠门岗 · 东', count: 12 },
        { name: '海珠门岗 · 西', count: 12 },
        { name: '海珠仓库', count: 32 },
        { name: '天河城 4F', count: 24 },
        { name: '高速 K127', count: 6 },
        { name: '停车场 B2', count: 18 },
        { name: '白云机场 2F', count: 22 }
      ],
      hours: ['00', '02', '04', '06', '08', '10', '12', '14', '16', '18', '20', '22'],
      segments: [
        { start: '00:00', end: '08:00', size: '320 MB', tone: 'success', label: '常规', style: { left: '0%', width: '33%' } },
        { start: '08:00', end: '08:24', size: '8 MB', tone: 'warning', label: '移动侦测', style: { left: '33%', width: '1.5%' } },
        { start: '08:30', end: '12:30', size: '180 MB', tone: 'success', label: '常规', style: { left: '34%', width: '17%' } },
        { start: '12:30', end: '12:36', size: '5 MB', tone: 'error', label: '手动录像', style: { left: '52%', width: '0.6%' } },
        { start: '14:20', end: '17:20', size: '240 MB', tone: 'success', label: '常规', style: { left: '60%', width: '12%' } },
        { start: '20:00', end: '24:00', size: '380 MB', tone: 'info', label: '告警联动', style: { left: '83%', width: '17%' } }
      ],
      rows: [
        { no: '01', start: '2026-07-05 00:00:12', end: '2026-07-05 08:00:08', duration: '7 时 59 分', size: '320 MB', type: '常规录像', tone: 'success', trigger: '24×7' },
        { no: '02', start: '2026-07-05 08:00:10', end: '2026-07-05 08:24:30', duration: '24 分', size: '8 MB', type: '移动侦测', tone: 'warning', trigger: '移动侦测' },
        { no: '03', start: '2026-07-05 08:30:00', end: '2026-07-05 12:30:00', duration: '4 时', size: '180 MB', type: '常规录像', tone: 'success', trigger: '24×7' },
        { no: '04', start: '2026-07-05 12:30:00', end: '2026-07-05 12:36:00', duration: '6 分', size: '5 MB', type: '手动录像', tone: 'error', trigger: '管理员' },
        { no: '05', start: '2026-07-05 14:22:18', end: '2026-07-05 17:20:00', duration: '2 时 57 分', size: '240 MB', type: '常规录像', tone: 'success', trigger: '24×7' },
        { no: '06', start: '2026-07-05 20:00:00', end: '2026-07-05 23:59:59', duration: '3 时 59 分', size: '380 MB', type: '告警联动', tone: 'info', trigger: 'AI 越界检测' }
      ]
    }
  }
}
</script>

<style lang="scss" scoped>
.playback { display: grid; grid-template-columns: 240px 1fr; gap: 12px; padding: 16px 20px; min-height: calc(100vh - 56px - 38px); }
@media (max-width: 1024px) { .playback { grid-template-columns: 1fr; } }

.pb-tree { background: var(--bg-surface); border: var(--layout-border); border-radius: var(--layout-radius); padding: 12px; display: flex; flex-direction: column; gap: 12px; }
.pb-tree__head { display: flex; justify-content: space-between; align-items: center; }
.pb-tree__list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
.pb-tree__list li { display: flex; align-items: center; gap: 6px; padding: 6px 8px; border-radius: 4px; cursor: pointer; font-size: 12px; }
.pb-tree__list li:hover { background: var(--bg-hover); }
.pb-tree__list li.is-active { background: rgba(11,138,178,.10); color: var(--brand-primary-500); }

.pb-main { display: flex; flex-direction: column; gap: 12px; min-width: 0; }

.player { display: flex; flex-direction: column; gap: 10px; }
.player__viewport { position: relative; aspect-ratio: 16/9; background: #0c1422; border-radius: 6px; overflow: hidden; }
.player__poster { position: absolute; inset: 0; display: grid; place-items: center; color: rgba(255,255,255,0.6); font-size: 48px; }
.player__overlay-top, .player__overlay-bottom { position: absolute; left: 12px; right: 12px; display: flex; justify-content: space-between; align-items: center; padding: 12px 0; color: #fff; }
.player__overlay-top { top: 0; }
.player__overlay-bottom { bottom: 0; }
.player__controls { display: flex; align-items: center; gap: 8px; padding: 4px 8px; background: var(--bg-elevated); border-radius: 6px; }
.player__btn { width: 28px; height: 28px; display: grid; place-items: center; border: 0; background: transparent; color: var(--text-secondary); cursor: pointer; border-radius: 4px; }
.player__btn:hover { background: var(--bg-hover); }
.player__btn.primary { background: var(--brand-primary-500); color: #fff; }
.player__btn.ghost { color: var(--text-tertiary); }
.player__progress { flex: 1; position: relative; height: 4px; background: var(--bg-overlay); border-radius: 2px; }
.player__progress-fill { position: absolute; top: 0; left: 0; height: 100%; background: var(--brand-primary-500); border-radius: 2px; }
.player__progress-marker { position: absolute; top: 50%; transform: translate(-50%, -50%); width: 10px; height: 10px; background: #fff; border-radius: 50%; box-shadow: 0 0 0 2px var(--brand-primary-500); }
.player__time { font-size: 11px; color: var(--text-tertiary); white-space: nowrap; }
.player__rate { background: var(--bg-surface); border: 1px solid var(--border-default); border-radius: 4px; padding: 2px 6px; font-size: 11px; }

.timeline__axis { display: flex; justify-content: space-between; padding: 0 4px 6px; font-size: 10px; }
.timeline__row { position: relative; height: 32px; background: var(--bg-elevated); border-radius: 4px; overflow: hidden; }
.timeline__seg { position: absolute; top: 0; bottom: 0; padding: 0 6px; display: flex; align-items: center; font-size: 10px; color: #fff; white-space: nowrap; }
.timeline__seg-label { overflow: hidden; text-overflow: ellipsis; }
.tone-success { background: var(--state-success); opacity: 0.85; }
.tone-warning { background: var(--state-warning); }
.tone-error { background: var(--state-error); }
.tone-info { background: var(--state-info); }
</style>
