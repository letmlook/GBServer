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
              <div class="thumb-cell" :class="{ 'thumb-cell--clickable': !!ch.thumb }">
                <!-- el-image 自带大图预览器（缩放/旋转/ESC）；
                     preview-teleported 避免被表格 overflow 裁掉。 -->
                <el-image
                  v-if="ch.thumb"
                  :src="ch.thumb"
                  :preview-src-list="[ch.thumb]"
                  :initial-index="0"
                  fit="cover"
                  preview-teleported
                  hide-on-click-modal
                  :alt="ch.name ?? ch.channelId"
                  class="thumb-cell__img"
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
import { captureSnap, listSnapshots, snapshotKey } from '@/api/live'
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
    // 进二级页后铺上已保存的缩略图（一次批量请求，先显示占位再替换）
    void loadThumbs(channels.value)
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
 * 给某个设备下的通道铺上**已保存的**缩略图。
 *
 * 缩略图由后端在每次点播时自动抓帧落盘，这里只批量读存量 ——
 * 一次请求搞定，不碰 ZLM、不唤醒设备。没存过的通道保持占位图标，
 * 等它被点播过一次自然就有了。
 */
async function loadThumbs(list: any[]) {
  const keys = list
    .filter((ch) => ch?.deviceId && ch?.channelId)
    .map((ch) => snapshotKey(ch.deviceId, ch.channelId))
  if (keys.length === 0) return
  try {
    const res = await listSnapshots(keys)
    const map = res?.data ?? {}
    for (const ch of list) {
      if (!ch?.deviceId || !ch?.channelId) continue
      const url = map[snapshotKey(ch.deviceId, ch.channelId)]
      if (url) ch.thumb = url
    }
  } catch {
    // 静默：全部保持占位图标
  }
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

/**
 * 「抓图」按钮：让后端立刻从该通道当前流里抓一帧、覆盖保存为缩略图，
 * 然后取回新 URL 弹预览窗。要求该通道当前**有活跃的流**（按需推流）。
 */
async function onSnapChannel(channelRow: any) {
  const deviceId = currentDeviceId.value
  const channelId = channelRow.channelId ?? channelRow.gbDeviceId ?? channelRow.deviceId
  const key = snapshotKey(deviceId, channelId)
  try {
    await captureSnap(deviceId, channelId)
    const res = await listSnapshots([key])
    const snapUrl = res?.data?.[key] ?? ''
    if (snapUrl) {
      // 同步缩略图到对应行
      channelRow.thumb = snapUrl
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

/**
 * 播放对话框回传的缩略图。
 *
 * `auto=true` 是"点播后后端自动抓的那一帧" —— 只用来填该行的缩略图，
 * **不弹预览窗口**（用户只是点了播放，不该被一个抓图浮窗打断）。
 * 用户在对话框里**手动点「抓图」**时才把图推进预览浮窗展示。
 */
function onPlaySnap(snapUrl: string, payload?: { auto?: boolean }) {
  if (!playingChannel.value || !snapUrl) return
  const { deviceId, channelId, name } = playingChannel.value

  // 同步缩略图到当前二级页的通道行（自动/手动都要做）
  const row = channels.value.find((c) => c.channelId === channelId)
  if (row) row.thumb = snapUrl

  if (payload?.auto) return

  snapItems.value.unshift({
    deviceId,
    channelId,
    name: name ?? channelId,
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
/* 有图时给"可点击放大"的鼠标反馈 */
.thumb-cell--clickable { cursor: zoom-in; }
.thumb-cell__img {
  width: 100%; height: 100%; display: block;
}
/* el-image 内部包了一层 div，尺寸要跟着撑满 */
.thumb-cell__img :deep(img) {
  width: 100%; height: 100%; object-fit: cover;
}
.thumb-cell__placeholder {
  color: var(--text-tertiary);
  display: flex; align-items: center; justify-content: center;
  width: 100%; height: 100%;
}
</style>