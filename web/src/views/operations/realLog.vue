<template>
  <div class="real-log-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">实时日志</h1>
        <p class="page-subtitle">{{ events.length }} 条 · 错误 {{ errors }} · 警告 {{ warns }}</p>
      </div>
      <div class="page-actions">
        <el-switch v-model="paused" active-text="暂停" />
        <el-switch v-model="wrap" active-text="自动换行" />
        <el-select v-model="level" placeholder="级别" clearable style="width: 120px">
          <el-option label="INFO" value="INFO" />
          <el-option label="WARN" value="WARN" />
          <el-option label="ERROR" value="ERROR" />
          <el-option label="DEBUG" value="DEBUG" />
        </el-select>
        <el-button @click="onClear">清屏</el-button>
      </div>
    </div>

    <el-card class="log-card">
      <template #header>
        <div class="log-header">
          <span class="text-tertiary text-xs">tail -f 模拟</span>
          <span class="text-tertiary text-xs">订阅：SIP · ZLM · DB · JT · Storage · Auth</span>
          <div class="flex-1" />
          <span :class="['status', paused ? 'paused' : 'running']">
            <span :class="['gb-dot', paused ? 'gb-dot--warning' : 'gb-dot--success']" />
            {{ paused ? '已暂停' : '运行中' }}
          </span>
        </div>
      </template>

      <div ref="scroller" :class="['log-body', wrap ? 'is-wrap' : '']" v-auto-bottom>
        <div v-if="!events.length" class="empty">
          <el-empty description="暂无日志（请确保后端推送）" />
        </div>
        <div v-for="(l, i) in filteredEvents" :key="i" :class="['log-line', 'lv-' + l.tone]">
          <span class="log-time mono">{{ l.time }}</span>
          <span :class="['log-level', 'lv-' + l.tone]">{{ l.level }}</span>
          <span class="log-logger mono">{{ l.logger }}</span>
          <span class="log-msg">{{ l.message }}</span>
        </div>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { getLogList, type LogRecord } from '@/api/log'

const events = ref<LogRecord[]>([])
const paused = ref(false)
const wrap = ref(false)
const level = ref('')

const filteredEvents = computed(() => {
  if (!level.value) return events.value
  return events.value.filter((e) => (e.level ?? '').toUpperCase() === level.value)
})

const errors = computed(() => events.value.filter((e) => (e.level ?? '').toUpperCase() === 'ERROR').length)
const warns = computed(() => events.value.filter((e) => (e.level ?? '').toUpperCase() === 'WARN').length)

const scroller = ref<HTMLElement | null>(null)
let timer: ReturnType<typeof setInterval> | null = null

async function loadHistory() {
  try {
    const res = await getLogList({ page: 1, count: 200 })
    events.value = (res.data?.list ?? []).map((e) => ({ ...e, tone: toneOf(e.level) }))
  } catch {
    events.value = []
  }
}

function toneOf(level?: string): 'info' | 'warn' | 'error' | 'debug' {
  switch ((level ?? '').toUpperCase()) {
    case 'ERROR':
      return 'error'
    case 'WARN':
    case 'WARNING':
      return 'warn'
    case 'DEBUG':
      return 'debug'
    default:
      return 'info'
  }
}

function pushMockEvent() {
  // 真实日志需 WebSocket 推送；当前每 5s 从历史 API 取最近未显示的一条
  // 生产环境需后端 /api/log/stream WebSocket 推送，这里用 polling 兜底
  getLogList({ page: 1, count: 1 })
    .then((res) => {
      const latest = res.data?.list?.[0]
      if (!latest) return
      // 避免重复：与最后一条 time+level+message 相同则跳过
      const last = events.value[events.value.length - 1]
      const sameAsLast =
        last && last.time === latest.time && last.level === latest.level && last.message === latest.message
      if (sameAsLast) return
      events.value.push({ ...latest, tone: toneOf(latest.level) })
      if (events.value.length > 500) events.value.shift()
    })
    .catch(() => {
      // 后端无新数据时静默失败；不影响 UI
    })
}

function onClear() {
  events.value = []
}

onMounted(async () => {
  await loadHistory()
  timer = setInterval(() => {
    if (paused.value) return
    pushMockEvent()
    nextTick(() => {
      if (scroller.value) scroller.value.scrollTop = scroller.value.scrollHeight
    })
  }, 5000)
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
})

// 简易 v-auto-bottom 指令：事件触发后自动滚到底
const vAutoBottom = {
  updated(el: HTMLElement) {
    el.scrollTop = el.scrollHeight
  }
}
</script>

<style scoped>
.real-log-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.log-card { min-height: 600px; }
.log-header { display: flex; align-items: center; gap: 12px; }
.status { display: flex; align-items: center; gap: 4px; font-size: 12px; }
.log-body { background: #0f172a; color: #e2e8f0; font-family: ui-monospace, monospace; font-size: 12px; padding: 12px; height: 540px; overflow-y: auto; border-radius: 4px; white-space: pre; line-height: 1.6; }
.log-body.is-wrap { white-space: pre-wrap; word-break: break-all; }
.empty { display: flex; align-items: center; justify-content: center; height: 100%; }
.log-line { display: flex; gap: 8px; padding: 1px 0; }
.log-time { color: #94a3b8; min-width: 140px; }
.log-level { padding: 0 6px; border-radius: 3px; min-width: 50px; text-align: center; font-weight: 600; }
.log-logger { color: #cbd5e1; min-width: 100px; }
.log-msg { flex: 1; }
.lv-info .log-level { background: #1e3a8a; color: #dbeafe; }
.lv-warn .log-level { background: #854d0e; color: #fef3c7; }
.lv-error .log-level { background: #991b1b; color: #fee2e2; }
.lv-debug .log-level { background: #374151; color: #e5e7eb; }
.lv-error .log-msg { color: #fca5a5; }
.lv-warn .log-msg { color: #fde68a; }
.mono { font-family: ui-monospace, monospace; }
.text-tertiary { color: var(--el-text-color-secondary); }
.text-xs { font-size: 12px; }
.flex-1 { flex: 1; }
.gb-dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; }
.gb-dot--success { background: #16a34a; }
.gb-dot--warning { background: #f59e0b; }
</style>
