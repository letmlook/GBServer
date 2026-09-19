<template>
  <div class="cloud-record-page">
    <el-card class="filter-card">
      <el-form class="gb-query-row" :inline="true">
        <el-form-item label="设备">
          <el-input v-model="query.deviceId" placeholder="国标设备ID" />
        </el-form-item>
        <el-form-item label="通道">
          <el-input v-model="query.channelId" placeholder="国标通道ID" />
        </el-form-item>
        <el-form-item label="App">
          <el-input v-model="query.app" />
        </el-form-item>
        <el-form-item label="Stream">
          <el-input v-model="query.stream" />
        </el-form-item>
        <el-form-item label="时间">
          <!-- 起止日期用日历、时分用下拉（15 分钟档）；v-model 仍落在
               query.startTime / query.endTime 上，检索与本地时间格式化不变 -->
          <GbDateTimeRange v-model="timeRange" />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="loadData">查询</el-button>
        </el-form-item>
        <el-form-item class="gb-query-actions">
          <el-button @click="loadData">刷新</el-button>
          <el-button type="success" :disabled="!selection.length" @click="onDownloadZip">打包下载</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card>
      <el-table table-layout="auto" :data="rows" v-loading="loading" stripe border @selection-change="onSelection">
        <el-table-column type="selection" width="48" />
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="app" label="App" width="100" />
        <el-table-column prop="stream" label="Stream" min-width="160">
          <template #default="{ row }"><span class="mono">{{ row.stream }}</span></template>
        </el-table-column>
        <el-table-column label="开始" min-width="170">
          <template #default="{ row }">
            <span class="mono">{{ formatTs(row.startTime) }}</span>
          </template>
        </el-table-column>
        <el-table-column label="结束" min-width="170">
          <template #default="{ row }">
            <span class="mono">{{ formatTs(row.endTime) }}</span>
          </template>
        </el-table-column>
        <el-table-column label="设备/通道" min-width="220">
          <template #default="{ row }">
            <span class="mono">{{ row.deviceId || '-' }} / {{ row.channelId || '-' }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="size" label="大小" width="100">
          <template #default="{ row }">{{ formatSize(row.size) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="200" fixed="right">
          <template #default="{ row }">
            <el-button link type="primary" @click="onPlay(row)">播放</el-button>
            <el-button link type="primary" @click="onDownload(row)">下载</el-button>
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
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import GbDateTimeRange from '@/components/GbDateTimeRange/index.vue'
import {
  getCloudRecordList,
  deleteCloudRecord,
  getCloudRecordPlayPath,
  downloadCloudRecordZip,
  type CloudRecord
} from '@/api/cloudRecord'

const loading = ref(false)
const rows = ref<CloudRecord[]>([])
const total = ref(0)
const selection = ref<CloudRecord[]>([])

/** 本地时间的当天 00:00（offsetDays 为负表示往前） */
function dayStart(offsetDays = 0): Date {
  const now = new Date()
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() + offsetDays, 0, 0, 0, 0)
}

const query = reactive({
  page: 1,
  count: 20,
  deviceId: '',
  channelId: '',
  app: '',
  stream: '',
  // 默认查最近 7 天（起止都未填 = 不限时间），这里给初值让页面进入即有范围。
  // 取「7 天前的 00:00 ~ 今天 24:00」而不是 now-7d ~ now：时间控件是 15 分钟
  // 档的下拉，落在档位上的值才能正确回显（详见组件内注释）。
  startTime: dayStart(-7) as Date | undefined,
  endTime: dayStart(1) as Date | undefined
})

/** GbDateTimeRange 的 v-model 代理：值仍落在 query.startTime / query.endTime 上，
    下面的检索与 formatLocal 序列化都不用改。 */
const timeRange = computed<[Date, Date] | null>({
  get: (): [Date, Date] | null =>
    query.startTime && query.endTime ? [query.startTime, query.endTime] : null,
  set: (v: [Date, Date] | null) => {
    query.startTime = v?.[0]
    query.endTime = v?.[1]
  }
})

async function loadData() {
  loading.value = true
  try {
    const res = await getCloudRecordList({
      page: query.page,
      count: query.count,
      deviceId: query.deviceId,
      channelId: query.channelId,
      app: query.app,
      stream: query.stream,
      // 后端的时间格式是本地 `yyyy-MM-dd HH:mm:ss`；
      // 早期发 `toISOString()`（带毫秒和 Z）会被解析失败，选完时间列表就空了。
      startTime: formatLocal(query.startTime),
      endTime: formatLocal(query.endTime)
    })
    rows.value = res.data?.list ?? []
    total.value = res.data?.total ?? 0
  } finally {
    loading.value = false
  }
}

/** Date → 本地 `yyyy-MM-dd HH:mm:ss`（后端契约） */
function formatLocal(d?: Date): string | undefined {
  if (!d) return undefined
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(
    d.getMinutes()
  )}:${p(d.getSeconds())}`
}

/** 毫秒时间戳 → 本地可读时间 */
function formatTs(ms?: number): string {
  if (!ms) return '-'
  return formatLocal(new Date(Number(ms))) ?? '-'
}

function formatSize(byte?: number): string {
  if (!byte) return '-'
  const units = ['B', 'KB', 'MB', 'GB']
  let v = byte
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(1)} ${units[i]}`
}

function onSelection(arr: CloudRecord[]) {
  selection.value = arr
}

async function onPlay(row: CloudRecord) {
  try {
    const res = await getCloudRecordPlayPath(row.id ?? 0)
    const data = (res.data as any) ?? {}
    const url: string = data.httpPath || data.playPath || data.filePath || ''
    if (url) {
      window.open(url, '_blank')
    } else {
      ElMessage.warning('该录像无可播放路径，请确认 ZLM 录像已生成')
    }
  } catch (e: any) {
    ElMessage.error(e?.message ?? '获取播放路径失败')
  }
}

async function onDownload(row: CloudRecord) {
  try {
    const res = await downloadCloudRecordZip([row.id ?? 0])
    const data = (res.data as any) ?? {}
    const url: string = data.url ?? data.downloadUrl ?? ''
    if (url) {
      window.open(url, '_blank')
      ElMessage.success('已开始下载 ZIP')
    } else {
      ElMessage.warning('后端未返回 ZIP URL，请检查 ZLM 存储配置')
    }
  } catch (e: any) {
    ElMessage.error(e?.message ?? 'ZIP 下载请求失败')
  }
}

async function onDelete(row: CloudRecord) {
  await ElMessageBox.confirm('确认删除该云端录像？', '确认', { type: 'warning' })
  try {
    // DELETE + {ids:[...]}（后端从 body 读 ids；早期用 GET 会 405）
    const res = await deleteCloudRecord([row.id ?? 0])
    const failed = res.data?.failed ?? []
    if (failed.length > 0) {
      ElMessage.warning(`部分录像删除失败：${failed.join(', ')}`)
    } else {
      ElMessage.success('已删除')
    }
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '删除失败')
  }
}

async function onDownloadZip() {
  if (selection.value.length === 0) {
    ElMessage.warning('请先选择要下载的录像')
    return
  }
  try {
    // 必须是数字主键（后端按 i64 解析 ids）
    const res = await downloadCloudRecordZip(selection.value.map((r) => r.id ?? 0))
    const url = (res.data as any)?.url
    if (url) window.open(url, '_blank')
    else ElMessage.warning('后端未返回 ZIP URL，请检查 ZLM 存储配置')
  } catch (e: any) {
    ElMessage.error(e?.message ?? 'ZIP 下载请求失败')
  }
}

onMounted(loadData)
</script>

<style scoped>
.cloud-record-page { padding: 16px; }
.filter-card { margin-bottom: 12px; }
.pagination { margin-top: 16px; justify-content: flex-end; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: var(--text-sm); }
</style>
