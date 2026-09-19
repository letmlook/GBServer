<template>
  <div class="alarm-page">
    <el-card class="filter-card">
      <el-form class="gb-query-row" :inline="true">
        <el-form-item label="关键字">
          <el-input v-model="query.query" placeholder="设备ID / 描述" clearable @keyup.enter="loadData" />
        </el-form-item>
        <el-form-item label="时间">
          <!-- 日期用日历、起止时分用下拉（15 分钟档），见组件内注释 -->
          <GbDateTimeRange v-model="timeRange" />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="loadData">查询</el-button>
        </el-form-item>
        <el-form-item class="gb-query-actions">
          <el-button @click="loadData">刷新</el-button>
          <el-button type="danger" :disabled="!selection.length" @click="onBatchClear">批量清除</el-button>
          <el-button type="warning" @click="onClearByFilter">按条件清空</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card>
      <el-table table-layout="auto" :data="rows" v-loading="loading" stripe border @selection-change="onSelection">
        <!-- 空状态区分两种情况：否则用户无法判断「真没有告警」还是「被筛选条件筛掉了」 -->
        <template #empty>
          <div v-if="timeRange" class="empty-hint">
            当前时间范围内没有告警；<el-button link type="primary" @click="clearTimeRange">清空时间筛选</el-button>可查看全部
          </div>
          <div v-else class="empty-hint">暂无告警记录</div>
        </template>
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
        <el-table-column label="级别" width="120">
          <template #default="{ row }">{{ alarmPriorityLabel(row.alarmPriority) }}</template>
        </el-table-column>
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
import GbDateTimeRange from '@/components/GbDateTimeRange/index.vue'
import {
  getAlarmList,
  deleteAlarm,
  deleteAlarms,
  clearAlarms,
  handleAlarm,
  alarmPriorityLabel,
  type Alarm
} from '@/api/alarm'

const loading = ref(false)
const rows = ref<Alarm[]>([])
const total = ref(0)
const selection = ref<Alarm[]>([])
// 默认查最近 7 天；用户手动清空则不限时间。
// 默认**不加时间过滤**。
//
// 此前默认填「最近 7 天」，于是页面一进来就带着 beginTime/endTime 去查：
// 只要告警都早于 7 天，列表就是「暂无数据」，而控制台的「最近告警」走同一个
// 接口却不带时间参数、照样有数据 —— 两处不一致，且用户无法从界面上看出
// 「是真的没有告警」还是「被默认筛选筛掉了」。
// 列表本身按时间倒序分页，全量展示没有负担；需要收窄时用户自己选时间即可。
const timeRange = ref<[Date, Date] | null>(null)

const query = reactive({
  page: 1,
  count: 20,
  query: '',
  startTime: undefined as string | undefined,
  endTime: undefined as string | undefined,
  alarmType: undefined as string | undefined
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

/// 清空时间筛选并重新查询（空状态里的快捷操作）
function clearTimeRange() {
  timeRange.value = null // watch 会把 startTime/endTime 置空
  query.page = 1
  loadData()
}

async function loadData() {
  loading.value = true
  try {
    const res = await getAlarmList({
      page: query.page,
      count: query.count,
      query: query.query,
      // 后端参数名是 beginTime
      beginTime: query.startTime,
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
    `设备: ${row.deviceId}\n通道: ${row.channelId}\n级别: ${alarmPriorityLabel(row.alarmPriority)}\n` +
      `类型: ${row.alarmType}\n时间: ${row.alarmTime}\n描述: ${row.alarmDescription}\n` +
      `处理: ${row.handled ? `${row.handleUser ?? ''} ${row.handleTime ?? ''}` : '未处理'}` +
      (row.handleResult ? `\n处理结论: ${row.handleResult}` : ''),
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

/**
 * 「清除」= 删除这一条。
 *
 * 早期这里调的是 `/api/alarm/clear`（GET）：该路由只注册了 DELETE，
 * 必然 405；而且后端那个 handler 是**无条件清空整张表**——一旦方法对上，
 * 点单行「清除」会把所有设备的告警一起删掉。
 */
async function onClear(row: Alarm) {
  await ElMessageBox.confirm(`确认清除该报警？`, '确认', { type: 'warning' })
  try {
    await deleteAlarm(row.id ?? 0)
    ElMessage.success('已清除')
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '清除失败')
  }
}

async function onDelete(row: Alarm) {
  await ElMessageBox.confirm(`确认删除该报警？`, '确认', { type: 'warning' })
  await deleteAlarm(row.id ?? 0)
  ElMessage.success('已删除')
  loadData()
}

/**
 * 「批量清除」= 删除选中的若干条（`DELETE /api/alarm/delete` + 裸数组 body）。
 * 早期是 `POST /api/alarm/batch` + `{ids, action}`：方法不匹配 405，
 * 且后端不认 `action`（"清除"会被当成永久删除）。
 */
async function onBatchClear() {
  if (selection.value.length === 0) {
    ElMessage.warning('请先选择要清除的告警')
    return
  }
  await ElMessageBox.confirm(`确认清除选中的 ${selection.value.length} 条？`, '确认', { type: 'warning' })
  try {
    await deleteAlarms(selection.value.map((r) => r.id ?? 0))
    ElMessage.success('已批量清除')
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '批量清除失败')
  }
}

/**
 * 「按条件清空」= `DELETE /api/alarm/clear`：清空当前筛选条件下的全部告警
 * （不带条件就是清空全部，二次确认里写清楚条数）。
 */
async function onClearByFilter() {
  const scope = query.query || query.startTime || query.endTime || query.alarmType ? '当前筛选条件下的' : '全部'
  await ElMessageBox.confirm(`确认清空${scope}告警？此操作不可恢复。`, '确认', { type: 'warning' })
  try {
    const res = await clearAlarms({
      query: query.query || undefined,
      beginTime: query.startTime || undefined,
      endTime: query.endTime || undefined,
      alarmType: query.alarmType || undefined
    })
    ElMessage.success(`已清空 ${res.data?.cleared ?? 0} 条`)
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '清空失败')
  }
}

onMounted(() => {
  // 默认时间范围在 reactive 里已经设过，但需在首次加载前把时间同步到 query。
  // 否则第一次 loadData() 不会带时间条件，与"默认 7 天"语义不符。
  if (timeRange.value) {
    query.startTime = timeRange.value[0].toISOString()
    query.endTime = timeRange.value[1].toISOString()
  }
  loadData()
})
</script>

<style scoped>
.alarm-page { padding: 16px; }
.filter-card { margin-bottom: 12px; }
.pagination { margin-top: 16px; justify-content: flex-end; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: var(--text-sm); }
</style>
<style lang="scss" scoped>
.empty-hint {
  padding: 24px 12px;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}
</style>
