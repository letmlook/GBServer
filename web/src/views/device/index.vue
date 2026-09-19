<template>
  <div class="device-page">
    <div v-if="!currentDeviceId">
      <div class="page-header">
        <div>
          <h1 class="page-title">国标设备</h1>
          <p class="page-subtitle">GB/T 28181 设备注册 · 心跳保活 · 目录同步</p>
        </div>
        <div class="page-actions">
          <el-button @click="loadData">刷新</el-button>
          <el-button :icon="Plus" type="primary" @click="onAdd">新增设备</el-button>
        </div>
      </div>

      <GbSearchForm :model="query" @search="loadData" @reset="resetQuery">
        <template #default="{ model }">
          <el-form-item label="关键字">
            <el-autocomplete
              :model-value="(model as any).query"
              @update:model-value="(v) => ((model as any).query = String(v ?? ''))"
              :fetch-suggestions="searchSuggest"
              placeholder="国标ID / 名称 / IP（输即搜）"
              clearable
              style="width: 280px"
              @select="onSelectSuggest"
              @keyup.enter="loadData"
            />
          </el-form-item>
          <el-form-item label="状态">
            <el-select :model-value="(model as any).status" @update:model-value="(v: string) => (model as any).status = v" placeholder="全部" clearable style="width: 140px">
              <el-option label="在线" value="ON" />
              <el-option label="离线" value="OFF" />
              <el-option label="全部" value="" />
            </el-select>
          </el-form-item>
        </template>
      </GbSearchForm>

      <el-card class="table-card">
        <el-table :data="rows" v-loading="loading" stripe border>
          <el-table-column type="index" label="#" width="50" />
          <el-table-column prop="deviceId" label="国标ID" min-width="180">
            <template #default="{ row }">
              <el-link type="primary" :underline="false" @click="showChannels(row)">
                <span class="mono">{{ row.deviceId }}</span>
              </el-link>
            </template>
          </el-table-column>
          <el-table-column prop="name" label="名称" min-width="140" show-overflow-tooltip />
          <el-table-column prop="manufacturer" label="厂家" width="100" show-overflow-tooltip />
          <el-table-column prop="model" label="型号" width="100" show-overflow-tooltip />
          <el-table-column prop="ip" label="IP" width="120">
            <template #default="{ row }"><span class="mono">{{ row.ip }}</span></template>
          </el-table-column>
          <el-table-column label="信令" width="100">
            <template #default="{ row }">
              <el-tag
                v-if="row.transport"
                :type="row.transport === 'TCP' ? 'warning' : 'success'"
                size="small"
              >{{ row.transport }}</el-tag>
              <span v-else class="text-tertiary">-</span>
            </template>
          </el-table-column>
          <el-table-column label="流模式" min-width="120">
            <template #default="{ row }">
              <el-tag
                v-if="row.streamMode"
                :type="row.streamMode === 'UDP' ? 'success' : 'warning'"
                size="small"
              >{{ row.streamMode }}</el-tag>
              <span v-else class="text-tertiary">-</span>
            </template>
          </el-table-column>
          <el-table-column label="在线" width="80">
            <template #default="{ row }">
              <el-tag :type="isOnline(row) ? 'success' : 'info'" size="small">
                {{ isOnline(row) ? '在线' : '离线' }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="channelCount" label="通道数" width="80" />
          <el-table-column label="操作" width="320" fixed="right">
            <template #default="{ row }">
              <el-button link type="primary" @click="showChannels(row)">通道</el-button>
              <el-button link type="primary" @click="onSync(row)">同步</el-button>
              <el-button link type="primary" @click="onEdit(row)">编辑</el-button>
              <el-button link :type="isOnline(row) ? 'warning' : 'success'" @click="onGuard(row)">
                {{ isOnline(row) ? '撤防' : '布防' }}
              </el-button>
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

    <div v-else>
      <el-page-header @back="currentDeviceId = ''">
        <template #content>
          <span class="page-title">通道 · {{ currentDeviceId }}</span>
        </template>
      </el-page-header>
      <el-card class="table-card" style="margin-top: 12px">
        <el-table :data="channels" v-loading="channelLoading" stripe border>
          <el-table-column label="缩略图" width="120" align="center">
            <template #default="{ row: ch }">
              <div class="thumb-cell">
                <img
                  v-if="ch.thumb"
                  :src="ch.thumb"
                  class="thumb-cell__img"
                  :alt="ch.name ?? ch.channelId"
                  loading="lazy"
                />
                <div v-else class="thumb-cell__placeholder">
                  <el-icon :size="18"><VideoCameraFilled /></el-icon>
                </div>
              </div>
            </template>
          </el-table-column>
          <el-table-column type="index" label="#" width="50" />
          <el-table-column prop="channelId" label="通道ID" min-width="180">
            <template #default="{ row: ch }"><span class="mono">{{ ch.channelId }}</span></template>
          </el-table-column>
          <el-table-column prop="name" label="通道名称" min-width="160" show-overflow-tooltip />
          <el-table-column prop="manufacturer" label="厂家" width="100" show-overflow-tooltip />
          <el-table-column label="在线" width="80">
            <template #default="{ row: ch }">
              <el-tag :type="ch.status === 'ON' ? 'success' : 'info'" size="small">
                {{ ch.status === 'ON' ? '在线' : '离线' }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="civilCode" label="行政区划" width="100" />
          <el-table-column label="地址" min-width="200" show-overflow-tooltip>
            <template #default="{ row: ch }">
              {{ formatAddress(ch.address) }}
            </template>
          </el-table-column>
          <el-table-column prop="subCount" label="子通道" width="80" />
          <el-table-column label="操作" width="240" fixed="right">
            <template #default="{ row: ch }">
              <el-button link type="primary" @click="openPlay(currentDeviceId, ch)">播放</el-button>
              <el-button link type="primary" @click="onSnapChannel(ch)">抓图</el-button>
            </template>
          </el-table-column>
        </el-table>
      </el-card>
    </div>

    <device-edit-dialog v-model="editVisible" :device="currentRow" @saved="loadData" />

    <!-- 通道播放对话框：取代跳转 /live -->
    <ChannelPlayDialog
      v-model="playVisible"
      :channel="playingChannel"
      @snap="onPlaySnap"
    />

    <SnapPreview v-model="snapVisible" :items="snapItems" @clear="snapItems = []" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { Plus, VideoCameraFilled } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { queryDevices, deleteDevice, sync, setGuard, resetGuard, queryChannels } from '@/api/device'
import { playSnap, queryStreams } from '@/api/live'
import GbSearchForm from '@/components/GbSearchForm/index.vue'
import DeviceEditDialog from './EditDialog.vue'
import SnapPreview from '@/components/SnapPreview/index.vue'
import ChannelPlayDialog from '@/components/ChannelPlayDialog/index.vue'

const loading = ref(false)
const channelLoading = ref(false)
const rows = ref<any[]>([])
const channels = ref<any[]>([])
const total = ref(0)
const currentDeviceId = ref('')
const currentRow = ref<any>({})
const editVisible = ref(false)

const snapVisible = ref(false)
const snapItems = ref<{ deviceId: string; channelId: string; name: string; snapUrl: string; time: number }[]>([])

const playVisible = ref(false)
const playingChannel = ref<{ deviceId: string; channelId: string; name: string } | null>(null)

const query = reactive({
  page: 1,
  count: 20,
  query: '',
  status: ''
})

function isOnline(row: any): boolean {
  return (
    row?.onLine === true ||
    row?.onLine === 1 ||
    row?.online === true ||
    row?.online === 1 ||
    row?.status === 'ON'
  )
}

/**
 * 安装地址占位：null/空/'Address' 默认值都显示 '-'。
 */
function formatAddress(v: unknown): string {
  if (v === null || v === undefined) return '-'
  const s = String(v).trim()
  if (!s) return '-'
  if (s.toLowerCase() === 'address') return '-'
  return s
}

async function loadData() {
  loading.value = true
  try {
    const res = await queryDevices({
      page: query.page,
      count: query.count,
      query: query.query,
      status: query.status
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
  query.status = ''
  query.page = 1
  loadData()
}

/**
 * 点"通道"或国标ID文字 → 进入该设备的通道二级页。
 * 这是恢复之前的交互：列表 ↔ 详情 二级切换。
 */
async function showChannels(row: any) {
  currentDeviceId.value = row.deviceId ?? ''
  channelLoading.value = true
  try {
    const res = await queryChannels(row.deviceId ?? '', { page: 1, count: 500 })
    channels.value = res.data?.list ?? []
    // 进二级页后批量抓缩略图（限制 6 并发），无需等所有图都回来就显示占位
    void refreshThumbs(channels.value)
  } catch {
    channels.value = []
  } finally {
    channelLoading.value = false
  }
}

/**
 * 批量抓缩略图（同 channel/index.vue 的实现）：
 * - 限并发 6
 * - URL 加 ?t=ms cache-busting
 * - 失败静默
 */
/**
 * 给某个设备下的通道批量抓缩略图。
 *
 * **只对当前有活跃流的通道抓图** —— 国标设备按需推流，没人在拉流时 ZLM
 * 里根本没有这路流，`getSnap` 必然失败且每次要白等超时 10s。先取一次
 * 活跃流清单，只对命中的通道发请求；其余保持占位图标，用户点播放后
 * 由 [ChannelPlayDialog] 自动回填。
 */
async function refreshThumbs(list: any[]) {
  let activeKeys = new Set<string>()
  try {
    const res = await queryStreams({ page: 1, count: 1000 })
    const streams = res.data?.list ?? []
    activeKeys = new Set(
      streams
        .filter((s: any) => s.deviceId && s.channelId)
        .map((s: any) => `${s.deviceId}_${s.channelId}`)
    )
  } catch {
    return
  }

  const tasks: Promise<void>[] = []
  const pool = new Set<Promise<void>>()
  const CONCURRENCY = 6
  for (const ch of list) {
    if (!ch?.deviceId || !ch?.channelId) continue
    if (!activeKeys.has(`${ch.deviceId}_${ch.channelId}`)) continue
    const p = (async () => {
      try {
        const res = await playSnap(ch.deviceId, ch.channelId)
        const url = res?.data?.snapUrl
        if (url) ch.thumb = `${url}${url.includes('?') ? '&' : '?'}t=${Date.now()}`
      } catch {
        // 静默：保持占位图标
      }
    })()
    pool.add(p)
    p.finally(() => pool.delete(p))
    if (pool.size >= CONCURRENCY) {
      await Promise.race(pool)
    }
    tasks.push(p)
  }
  await Promise.allSettled(tasks)
}

const rowsSuggestion = ref<any[]>([])

function searchSuggest(kw: string, cb: (arr: any[]) => void) {
  const k = (kw ?? '').trim().toLowerCase()
  if (!k) {
    cb(rowsSuggestion.value.slice(0, 30).map((r) => ({ value: formatLabel(r) })))
    return
  }
  cb(
    rowsSuggestion.value
      .filter(
        (r) =>
          String(r.deviceId ?? '').toLowerCase().includes(k) ||
          String(r.name ?? '').toLowerCase().includes(k) ||
          String(r.ip ?? '').toLowerCase().includes(k)
      )
      .slice(0, 30)
      .map((r) => ({ value: formatLabel(r) }))
  )
}

function formatLabel(r: any): string {
  return `${r.deviceId} · ${r.name ?? '-'} (${r.ip ?? '-'})`
}

function onSelectSuggest() {
  loadData()
}

function onAdd() {
  currentRow.value = {}
  editVisible.value = true
}

function onEdit(row: any) {
  currentRow.value = { ...row }
  editVisible.value = true
}

async function onSync(row: any) {
  await sync(row.deviceId ?? '')
  ElMessage.success('同步请求已发送')
}

async function onGuard(row: any) {
  try {
    if (isOnline(row)) {
      await resetGuard(row.deviceId ?? '')
      ElMessage.success('撤防指令已发送')
    } else {
      await setGuard(row.deviceId ?? '')
      ElMessage.success('布防指令已发送')
    }
  } catch (e: any) {
    ElMessage.error(e?.message ?? '指令发送失败')
  }
}

async function onDelete(row: any) {
  await ElMessageBox.confirm(`确认删除设备 ${row.name ?? row.deviceId} ？此操作会级联删除该设备的全部通道与录像计划`, '危险操作', {
    type: 'error'
  })
  await deleteDevice(row.deviceId ?? '')
  ElMessage.success('已删除')
  loadData()
}

/**
 * 打开播放对话框：不再跳转 /live，停在设备列表上下文。
 */
function openPlay(deviceId: string, channelRow: any) {
  playingChannel.value = {
    deviceId,
    channelId: channelRow.channelId ?? channelRow.gbDeviceId ?? channelRow.deviceId,
    name: channelRow.name ?? channelRow.channelId
  }
  playVisible.value = true
}

async function onSnapChannel(channelRow: any) {
  const deviceId = currentDeviceId.value
  const channelId = channelRow.channelId ?? channelRow.gbDeviceId ?? channelRow.deviceId
  try {
    const res = await playSnap(deviceId, channelId)
    const snapUrl = res.data?.snapUrl ?? ''
    if (snapUrl) {
      // 同步缩略图到对应行
      channelRow.thumb = `${snapUrl}${snapUrl.includes('?') ? '&' : '?'}t=${Date.now()}`
      snapItems.value.unshift({
        deviceId,
        channelId,
        name: channelRow.name ?? channelId,
        snapUrl,
        time: Date.now()
      })
      if (snapItems.value.length > 8) snapItems.value.length = 8
      snapVisible.value = true
    }
    ElMessage.success('抓图已保存')
  } catch (e: any) {
    ElMessage.error(e?.message ?? '抓图失败')
  }
}

function onPlaySnap(snapUrl: string) {
  if (!playingChannel.value || !snapUrl) return
  // 同步缩略图到当前二级页的通道行
  const row = channels.value.find(
    (c) => c.channelId === playingChannel.value!.channelId
  )
  if (row) row.thumb = `${snapUrl}${snapUrl.includes('?') ? '&' : '?'}t=${Date.now()}`
  snapItems.value.unshift({
    deviceId: playingChannel.value.deviceId,
    channelId: playingChannel.value.channelId,
    name: playingChannel.value.name ?? playingChannel.value.channelId,
    snapUrl,
    time: Date.now()
  })
  if (snapItems.value.length > 8) snapItems.value.length = 8
  snapVisible.value = true
}

onMounted(async () => {
  await loadData()
  queryDevices({ page: 1, count: 200 })
    .then((r) => (rowsSuggestion.value = r.data?.list ?? []))
    .catch(() => {})
})
</script>

<style scoped>
.device-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 16px; }
.page-title { font-size: var(--text-xl); font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.table-card { min-height: 400px; overflow-x: auto; }
.pagination { margin-top: 16px; justify-content: flex-end; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }

.text-tertiary { color: var(--text-tertiary); }
.text-xs { font-size: 12px; }

/* 缩略图列：16:9 缩略图，加载前显示摄像头占位符 */
.thumb-cell {
  width: 96px;
  aspect-ratio: 16 / 9;
  border-radius: 4px;
  overflow: hidden;
  background: var(--bg-elevated);
  display: flex; align-items: center; justify-content: center;
  margin: 0 auto;
  border: 1px solid var(--border-subtle);
}
.thumb-cell__img {
  width: 100%; height: 100%; object-fit: cover; display: block;
}
.thumb-cell__placeholder {
  color: var(--text-tertiary);
  display: flex; align-items: center; justify-content: center;
  width: 100%; height: 100%;
}
</style>