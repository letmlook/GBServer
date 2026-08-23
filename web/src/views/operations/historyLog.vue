<template>
  <div class="history-log-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">历史日志</h1>
        <p class="page-subtitle">{{ total }} 条 · 支持按时间/级别/关键字检索</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button @click="onExport">导出</el-button>
      </div>
    </div>

    <el-card class="filter-card">
      <el-form :inline="true">
        <el-form-item label="关键字">
          <el-input v-model="query.query" placeholder="消息关键字" clearable @keyup.enter="loadData" />
        </el-form-item>
        <el-form-item label="级别">
          <el-select v-model="query.level" placeholder="全部" clearable style="width: 120px">
            <el-option label="INFO" value="INFO" />
            <el-option label="WARN" value="WARN" />
            <el-option label="ERROR" value="ERROR" />
            <el-option label="DEBUG" value="DEBUG" />
          </el-select>
        </el-form-item>
        <el-form-item label="时间">
          <el-date-picker v-model="timeRange" type="datetimerange" />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="loadData">查询</el-button>
          <el-button @click="resetQuery">重置</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card>
      <el-table :data="rows" v-loading="loading" stripe border>
        <el-table-column prop="time" label="时间" min-width="180">
          <template #default="{ row }"><span class="mono">{{ row.time }}</span></template>
        </el-table-column>
        <el-table-column label="级别" width="100">
          <template #default="{ row }">
            <el-tag :type="levelTagType(row.level)" size="small">{{ row.level }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="logger" label="Logger" min-width="160">
          <template #default="{ row }"><span class="mono">{{ row.logger }}</span></template>
        </el-table-column>
        <el-table-column prop="message" label="消息" min-width="280" show-overflow-tooltip />
        <el-table-column prop="thread" label="线程" min-width="120">
          <template #default="{ row }"><span class="mono">{{ row.thread }}</span></template>
        </el-table-column>
      </el-table>

      <el-pagination
        v-model:current-page="query.page"
        v-model:page-size="query.count"
        :total="total"
        :page-sizes="[20, 50, 100, 200]"
        layout="total, sizes, prev, pager, next, jumper"
        class="pagination"
        @current-change="loadData"
        @size-change="loadData"
      />
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { getLogList, type LogRecord } from '@/api/log'

const loading = ref(false)
const rows = ref<LogRecord[]>([])
const total = ref(0)
const timeRange = ref<[Date, Date] | null>(null)

const query = reactive({
  page: 1,
  count: 20,
  query: '',
  level: '',
  startTime: undefined as string | undefined,
  endTime: undefined as string | undefined
})

watch(timeRange, (v) => {
  if (v) {
    query.startTime = v[0].toISOString()
    query.endTime = v[1].toISOString()
  } else {
    query.startTime = undefined
    query.endTime = undefined
  }
})

async function loadData() {
  loading.value = true
  try {
    const res = await getLogList({
      page: query.page,
      count: query.count,
      query: query.query,
      level: query.level,
      startTime: query.startTime,
      endTime: query.endTime
    })
    rows.value = res.data?.list ?? []
    total.value = res.data?.total ?? 0
  } catch {
    rows.value = []
    total.value = 0
  } finally {
    loading.value = false
  }
}

function resetQuery() {
  query.query = ''
  query.level = ''
  timeRange.value = null
  query.page = 1
  loadData()
}

function levelTagType(level?: string): 'success' | 'warning' | 'danger' | 'info' {
  switch ((level ?? '').toUpperCase()) {
    case 'ERROR':
      return 'danger'
    case 'WARN':
    case 'WARNING':
      return 'warning'
    case 'DEBUG':
      return 'info'
    default:
      return 'success'
  }
}

function onExport() {
  ElMessage.info('通过 /api/log/list?format=csv 导出（当前接口已支持流式输出）')
}

onMounted(loadData)
</script>

<style scoped>
.history-log-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.filter-card { margin-bottom: 12px; }
.pagination { margin-top: 16px; justify-content: flex-end; }
.mono { font-family: ui-monospace, monospace; font-size: 12px; }
</style>
