<template>
  <div class="gb-page real-log">
    <div class="gb-page__header">
      <div>
        <h1 class="gb-page__title">实时日志 <span class="gb-dot gb-dot--success" style="margin-left:6px" /> <span class="text-tertiary text-xs">tail -f</span></h1>
        <p class="gb-page__subtitle">来自 14 个媒体节点、SIP 网关、数据库等 · 实时刷新 1s</p>
      </div>
      <div class="gb-page__actions">
        <el-switch v-model="paused" active-text="暂停" />
        <el-switch v-model="wrap" active-text="自动换行" />
        <select v-model="level" class="search-mini">
          <option value="">全部级别</option><option>INFO</option><option>WARN</option><option>ERROR</option>
        </select>
        <button class="gb-btn" @click="cleared = true">清屏</button>
        <button class="gb-btn gb-btn--danger">停止订阅</button>
      </div>
    </div>

    <article class="gb-card log-card" style="padding:0">
      <div class="log-toolbar">
        <span class="text-tertiary text-xs mono">{{ events.length }} 条</span>
        <span class="text-tertiary text-xs">·</span>
        <span class="text-tertiary text-xs">错误 <span class="text-error mono">{{ errors }}</span> · 警告 <span class="text-warning mono">{{ warns }}</span></span>
        <div class="flex-1" />
        <span class="text-tertiary text-xs">订阅：SIP · ZLM · DB · JT · Storage · Auth</span>
      </div>
      <div ref="scroller" class="log-body">
        <div v-for="(l, i) in events" :key="i" :class="['log-line', 'lv-' + l.tone]">
          <span class="log-time mono">{{ l.time }}</span>
          <span :class="['log-level', 'lv-' + l.tone]">{{ l.level }}</span>
          <span class="log-module">{{ l.module }}</span>
          <span class="log-text mono">{{ l.text }}</span>
        </div>
        <empty-state v-if="cleared" text="已清屏 · 等待新日志…" />
      </div>
    </article>
  </div>
</template>

<script>
import EmptyState from '@/components/EmptyState'

export default {
  name: 'RealLog',
  components: { EmptyState },
  data() {
    return {
      paused: false,
      wrap: false,
      level: '',
      cleared: false,
      errors: 0,
      warns: 0,
      events: [
        { time: '16:42:18.214', level: 'ERROR', tone: 'error', module: 'SIP', text: '[41042200001320000102] receive BYE timeout (3 retries left)' },
        { time: '16:42:17.911', level: 'INFO', tone: 'info', module: 'ZLM', text: 'stream open: rtmp://live.aliyun.com/live/cdn001 (192.168.4.18:38514)' },
        { time: '16:42:16.732', level: 'INFO', tone: 'info', module: 'DB', text: 'INSERT INTO gb_record_plan (name, type, start, end) VALUES (...)' },
        { time: '16:42:15.488', level: 'WARN', tone: 'warning', module: 'SIP', text: 'register timeout for sip-gw-beijing, will retry in 30s' },
        { time: '16:42:14.211', level: 'INFO', tone: 'info', module: 'JT', text: '[粤B·A8888] gps pos (113.27, 23.13) speed=78km/h' },
        { time: '16:42:12.044', level: 'INFO', tone: 'info', module: 'Auth', text: 'user ops-tianhe login ok' },
        { time: '16:42:10.892', level: 'INFO', tone: 'info', module: 'ZLM', text: 'edge-04 cpu=71% mem=78% (warning threshold reached)' },
        { time: '16:42:09.330', level: 'ERROR', tone: 'error', module: 'JT', text: '[粤B·A8888] gps signal lost for 480s, raising alarm' },
        { time: '16:42:08.110', level: 'INFO', tone: 'info', module: 'Cascade', text: 'send REGISTER sip:13010000002000000001@10.20.4.5' }
      ]
    }
  },
  mounted() {
    this.tick()
  },
  beforeDestroy() { if (this._t) clearInterval(this._t) },
  methods: {
    tick() {
      this._t = setInterval(() => {
        if (this.paused) return
        const samples = [
          { level: 'INFO', tone: 'info', module: 'SIP', text: `[${this.id()}] INVITE 200 OK from 10.21.4.118` },
          { level: 'INFO', tone: 'info', module: 'ZLM', text: `playback rtmp://${this.id()}/live/main opened` },
          { level: 'WARN', tone: 'warning', module: 'Storage', text: `disk usage 78% on /mnt/store-03` },
          { level: 'INFO', tone: 'info', module: 'DB', text: `SELECT COUNT(*) FROM gb_channel WHERE online=1 → 2915` },
          { level: 'ERROR', tone: 'error', module: 'ZLM', text: `stream close: client timeout (10.20.4.10)` }
        ]
        const s = samples[Math.floor(Math.random() * samples.length)]
        const now = new Date()
        const time = now.toTimeString().slice(0, 8) + '.' + String(now.getMilliseconds()).padStart(3, '0')
        this.events.push({ time, ...s })
        if (this.events.length > 200) this.events.shift()
        if (s.tone === 'error') this.errors++
        if (s.tone === 'warning') this.warns++
        this.$nextTick(() => {
          const el = this.$refs.scroller
          if (el) el.scrollTop = el.scrollHeight
        })
      }, 1100)
    },
    id() { return '44' + Math.floor(Math.random() * 1e15).toString().padStart(15, '0') }
  }
}
</script>

<style lang="scss" scoped>
.log-card { display: flex; flex-direction: column; height: calc(100vh - 56px - 38px - 92px); }
.log-toolbar { display: flex; gap: 12px; align-items: center; padding: 10px 14px; border-bottom: 1px solid var(--border-subtle); }
.log-body { flex: 1; overflow: auto; background: #0a0f17; color: #d6e0ec; font-family: var(--font-mono); font-size: 12px; padding: 6px 0; }
.log-line { display: grid; grid-template-columns: 110px 60px 90px 1fr; gap: 8px; padding: 2px 14px; align-items: center; }
.log-line:hover { background: rgba(255,255,255,0.04); }
.log-time { color: #5a6f87; }
.log-level { font-weight: 700; padding: 1px 6px; border-radius: 3px; text-align: center; font-size: 10px; }
.log-level.lv-info { background: rgba(2,132,199,.20); color: #5eb4d4; }
.log-level.lv-warning { background: rgba(234,138,12,.20); color: #ea8a0c; }
.log-level.lv-error { background: rgba(220,38,38,.20); color: #ef4444; }
.log-module { color: #9ad0e6; }
.log-text { white-space: pre; overflow: hidden; text-overflow: ellipsis; }

.search-mini { padding: 4px 8px; font-size: 11px; border: 1px solid var(--border-default); background: var(--bg-elevated); border-radius: 4px; color: var(--text-primary); outline: 0; }
.flex-1 { flex: 1; }
.text-error { color: var(--state-error); }
.text-warning { color: var(--state-warning); }
</style>
