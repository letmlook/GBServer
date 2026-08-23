<template>
  <div class="alarm-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">报警管理</h1>
        <p class="page-subtitle">设备报警事件 · 处理 / 清除 / 抓图</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button type="danger" @click="onBatchClear" :disabled="!selection.length">批量清除</el-button>
      </div>
    </div>

    <el-card class="filter-card">
      <el-form :inline="true">
        <el-form-item label="关键字">
          <el-input v-model="query.query" placeholder="设备ID / 描述" clearable @keyup.enter="loadData" />
        </el-form-item>
        <el-form-item label="时间">
          <el-date-picker v-model="timeRange" type="datetimerange" range-separator="-" />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="loadData">查询</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card>
      <el-table :data="rows" v-loading="loading" stripe border @selection-change="onSelection">
        <el-table-column type="selection" width="48" />
        <el-table-column prop="alarmTime" label="报警时间" min-width="180">
          <template #default="{ row }"><span class="mono">{{ row.alarmTime }}</span></template>
        </el-table-column>
        <el-table-column prop="deviceId" label="设备ID" min-width="180">
          <template #default="{ row }"><span class="mono">{{ row.deviceId }}</span></template>
        </el-table-column>
        <el-table-column prop="channelId" label="通道ID" min-width="180">
          <template #default="{ row }"><span class="mono">{{ row.channelId }}</span></template>
        </el-table-column>
        <el-table-column prop="alarmLevel" label="级别" width="100" />
        <el-table-column prop="alarmType" label="类型" width="120" />
        <el-table-column prop="alarmDescription" label="描述" min-width="240" show-overflow-tooltip />
        <el-table-column label="状态" width="100">
          <template #default="{ row }">
            <el-tag :type="row.handled ? 'success' : 'warning'" size="small">
              {{ row.handled ? '已处理' : '未处理' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="200" fixed="right">
          <template #default="{ row }">
            <el-button link type="primary" @click="onView(row)">查看</el-button>
            <el-button link type="success" :disabled="row.handled" @click="onHandle(row)">处理</el-button>
            <el-button link type="warning" @click="onClear(row)">清除</el-button>
            <el-button link type="danger" @click="onDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>

      <el-pagination
        v-model:current-page="query.page"
        v-model:page-size="query.count"
        :total="total"
        :page-sizes="[20, 50, 100]"
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
import { ElMessage, ElMessageBox } from 'element-plus'
import { getAlarmList, clearAlarm, deleteAlarm, handleAlarm, batchAlarm, type Alarm } from '@/api/alarm'

const loading = ref(false)
const rows = ref<Alarm[]>([])
const total = ref(0)
const selection = ref<Alarm[]>([])
const timeRange = ref<[Date, Date] | null>(null)

const query = reactive({
  page: 1,
  count: 20,
  query: '',
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
    const res = await getAlarmList({
      page: query.page,
      count: query.count,
      query: query.query,
      startTime: query.startTime,
      endTime: query.endTime
    })
    rows.value = res.data?.list ?? []
    total.value = res.data?.total ?? 0
  } finally {
    loading.value = false
  }
}

function onSelection(arr: Alarm[]) {
  selection.value = arr
}

function onView(row: Alarm) {
  ElMessageBox.alert(
    `设备: ${row.deviceId}\n通道: ${row.channelId}\n级别: ${row.alarmLevel}\n类型: ${row.alarmType}\n时间: ${row.alarmTime}\n描述: ${row.alarmDescription}`,
    '报警详情'
  )
}

async function onHandle(row: Alarm) {
  const { value } = await ElMessageBox.prompt('处理结果', '处理报警', {
    inputValidator: (v) => (v ? true : '请输入处理结果')
  })
  await handleAlarm({ id: row.id ?? 0, result: value })
  ElMessage.success('已处理')
  loadData()
}

async function onClear(row: Alarm) {
  await clearAlarm(row.id ?? 0)
  ElMessage.success('已清除')
  loadData()
}

async function onDelete(row: Alarm) {
  await ElMessageBox.confirm(`确认删除该报警？`, '确认', { type: 'warning' })
  await deleteAlarm(row.id ?? 0)
  ElMessage.success('已删除')
  loadData()
}

async function onBatchClear() {
  await ElMessageBox.confirm(`确认批量清除 ${selection.value.length} 条？`, '确认', { type: 'warning' })
  await batchAlarm({ ids: selection.value.map((r) => r.id ?? 0), action: 'clear' })
  ElMessage.success('已批量清除')
  loadData()
}

onMounted(loadData)
</script>

<style scoped>
.alarm-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.filter-card { margin-bottom: 12px; }
.pagination { margin-top: 16px; justify-content: flex-end; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
