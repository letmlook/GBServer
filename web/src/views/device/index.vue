<template>
  <div class="device-page">
    <div v-if="!currentDeviceId">
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
        <template #actions>
          <el-button @click="loadData">刷新</el-button>
          <el-button :icon="Plus" type="primary" @click="onAdd">新增设备</el-button>
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
          <!-- 不再单列 IP：设备端 IP 已经在下一列「信令地址」里以 ip:port
               的形式给出，单独再放一个 IP 列是重复信息、白占宽度。 -->
          <!-- 信令地址：设备端信令的注册来源 IP:端口。
               管理员用它对 NAT 映射、排查注册问题。
               （传输层协议 UDP/TCP 由「流模式」下拉体现，不在这里重复。） -->
          <el-table-column label="信令地址" min-width="150">
            <template #default="{ row }">
              <span v-if="row.ip" class="mono">{{ row.ip }}:{{ row.port ?? '-' }}</span>
              <span v-else class="text-tertiary">-</span>
            </template>
          </el-table-column>
          <!-- 流模式：可直接改。选定后后端会落库**并向下发 SIP
               DeviceControl/Transport 消息**，让设备按平台指定的模式协商收流。 -->
          <el-table-column label="流模式" width="168">
            <template #default="{ row }">
              <el-select
                :model-value="row.streamMode || STREAM_MODE_DEFAULT"
                size="small"
                :disabled="modeChanging === row.deviceId"
                popper-class="stream-mode-select"
                style="width: 100%"
                @change="(v: string) => onChangeStreamMode(row, v)"
              >
                <el-option
                  v-for="m in STREAM_MODES"
                  :key="m.value"
                  :label="m.label"
                  :value="m.value"
                >
                  <span>{{ m.label }}</span>
                  <span class="mode-hint">{{ m.hint }}</span>
                </el-option>
              </el-select>
            </template>
          </el-table-column>
          <el-table-column label="在线" width="80">
            <template #default="{ row }">
              <el-tag :type="isOnline(row) ? 'success' : 'info'" size="small">
                {{ isOnline(row) ? '在线' : '离线' }}
              </el-tag>
            </template>
          </el-table-column>
          <!-- 延迟：平台 → 设备 → 平台的 **SIP 往返时间**（由 SIP 探针量出来，
               不是设备心跳周期）。设备离线时不显示数字：离线设备没有探针在途，
               留着上次的样本会被误读成"当前延迟"。 -->
          <el-table-column label="延迟" width="136" align="center">
            <template #default="{ row }">
              <span v-if="!isOnline(row)" class="text-tertiary">-</span>
              <span
                v-else-if="probeIntervalSecs === 0"
                class="text-tertiary"
                title="延迟探针已关闭（sip.heartbeat.latency_probe_interval_secs = 0）"
              >未启用</span>
              <el-tooltip v-else :content="latencyTitle(row)" placement="top">
                <span class="latency-cell">
                  <span :class="['gb-dot', latencyDot(row)]" />
                  <span class="mono">{{ latencyText(row) }}</span>
                  <span class="text-tertiary latency-cell__q">{{ latencyQuality(row) }}</span>
                </span>
              </el-tooltip>
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
      <el-page-header @back="currentDeviceId = ''" />
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
import { onMounted, onUnmounted, reactive, ref } from 'vue'
import { Plus, VideoCameraFilled } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  queryDevices,
  deleteDevice,
  sync,
  setGuard,
  resetGuard,
  queryChannels,
  updateDeviceTransport,
  queryDeviceLatency,
  type DeviceLatency
} from '@/api/device'
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

/**
 * 流模式可选值。
 *
 * GB28181 的媒体流传输只有三种形态：
 *   * `UDP`         —— 平台被动收流（设备往平台开好的 RTP 端口推）
 *   * `TCP-PASSIVE` —— 平台被动：设备主动连平台的 TCP 端口推流
 *   * `TCP-ACTIVE`  —— 平台主动：平台去连设备的 TCP 端口拉流
 *
 * 值保持早期实现以来的拼写（`TCP-PASSIVE` / `TCP-ACTIVE`），后端
 * `/api/device/query/transport/:id/:mode` 就是按这个集合校验的。
 */
const STREAM_MODES = [
  { value: 'UDP', label: 'UDP', hint: '设备推流到平台（被动收流）' },
  { value: 'TCP-PASSIVE', label: 'TCP 被动', hint: '设备连接平台' },
  { value: 'TCP-ACTIVE', label: 'TCP 主动', hint: '平台连接设备' }
] as const

/** 设备没设过流模式时的默认值 —— 与后端 `DEFAULT_STREAM_MODE` 保持一致 */
const STREAM_MODE_DEFAULT = 'UDP'

/** 正在切换流模式的设备（用于禁用下拉 + 避免重复点击） */
const modeChanging = ref('')

/**
 * 切换某台设备的流模式。
 *
 * 后端做两件事（见 `device_stub::device_transport`）：
 *   1. 落库，后续点播都按新模式建流；
 *   2. 向设备下发 SIP `DeviceControl/Transport` 消息，**通知设备按平台指定
 *      的模式协商**收流。
 *
 * 设备不在线时只落库、不发 SIP —— 这时如实告诉用户"已保存，上线后生效"，
 * 不要谎报"已通知设备"。
 */
async function onChangeStreamMode(row: any, mode: string) {
  const deviceId = row?.deviceId
  if (!deviceId || mode === row.streamMode) return
  modeChanging.value = deviceId
  try {
    const res = (await updateDeviceTransport(deviceId, mode)) as unknown as {
      code: number
      data?: { sipSent?: boolean; sipError?: string | null }
    }
    row.streamMode = mode
    const sipSent = res?.data?.sipSent
    if (sipSent) {
      ElMessage.success(`流模式已切换为 ${mode}，已通知设备协商`)
    } else if (isOnline(row)) {
      // 在线却发不出去：把后端给的原因带出来，别只说"成功"
      ElMessage.warning(
        `流模式已保存为 ${mode}，但通知设备失败${res?.data?.sipError ? `：${res.data.sipError}` : ''}`
      )
    } else {
      ElMessage.success(`流模式已保存为 ${mode}（设备离线，上线后生效）`)
    }
  } catch (e: any) {
    ElMessage.error(e?.message ?? '切换流模式失败')
    // 失败时把下拉回滚到服务端的真实值，避免界面显示成"已改"
    loadData()
  } finally {
    modeChanging.value = ''
  }
}

const query = reactive({
  page: 1,
  count: 20,
  query: '',
  status: ''
})

/* ── 延迟（平台 ↔ 设备的 SIP 往返） ──
 *
 * 数据源是 `/device/query/latency`，后端每个探针周期（默认 15s）向每台
 * 在线设备发一条 SIP MESSAGE 并量往返时间。前端 5s 拉一次，比探针周期
 * 短 —— 保证设备刚测完就能显示出来，而不是最多等一个完整周期。
 *
 * 注册表在后端内存里，键是 deviceId；这里只缓存「当前页需要的那些行」，
 * 表里没有的行查不到就是"还没测出来"，显示测量中。
 */
const latencyMap = ref<Record<string, DeviceLatency>>({})
/** 探针周期（秒）。0 = 后端关闭了探针，前端据此显示"未启用"而不是"测量中"。 */
const probeIntervalSecs = ref(15)
let latencyTimer: number | null = null

const LATENCY_POLL_MS = 5_000

async function loadLatency() {
  // 二级页（通道列表）不显示设备延迟，别白拉接口。
  if (currentDeviceId.value) return
  const ids = rows.value.map((r) => r.deviceId).filter(Boolean)
  if (!ids.length) {
    latencyMap.value = {}
    return
  }
  try {
    const res = await queryDeviceLatency(ids)
    const map: Record<string, DeviceLatency> = {}
    for (const item of res?.data?.list ?? []) {
      if (item?.deviceId) map[item.deviceId] = item
    }
    latencyMap.value = map
    if (typeof res?.data?.probeIntervalSecs === 'number') {
      probeIntervalSecs.value = res.data.probeIntervalSecs
    }
  } catch {
    // 静默：延迟是附加信息，拉不到就保持上一次的值，不打断设备管理操作。
  }
}

/** 该行当前应有的延迟样本（没有 = 还没测出来）。 */
function latencyOf(row: any): DeviceLatency | undefined {
  return latencyMap.value[row?.deviceId]
}

/**
 * 延迟档位。阈值按 GB28181 的常见部署定：
 * 同局域网设备正常在个位数~几十毫秒；跨公网几百毫秒也常见，
 * 所以 80/200ms 分档，超过 200ms 才算"差"。
 */
function latencyLevel(row: any): 'unknown' | 'good' | 'fair' | 'poor' | 'lost' {
  const l = latencyOf(row)
  if (!l || l.samples === 0) return 'unknown'
  if (!l.ok) return 'lost'
  const v = l.rttMs ?? l.lastMs ?? 0
  if (v > 200) return 'poor'
  if (v > 80) return 'fair'
  return 'good'
}

/**
 * 档位 → 圆点样式 + 文案。
 *
 * 圆点用的是 styles/_utilities.scss 里的**全局** `gb-dot--*`
 * （success/warning/error），不带后缀时是灰色基态。
 * 不要照抄 Navbar 里的 `gb-dot--warn` / `gb-dot--err`：那两个是
 * Navbar.vue 的 scoped 类，在别的组件里命中不到，点会一直是灰的。
 */
const LEVEL_TEXT: Record<string, { dot: string; label: string }> = {
  unknown: { dot: '', label: '测量中' },
  good: { dot: 'gb-dot--success', label: '优' },
  fair: { dot: 'gb-dot--warning', label: '良' },
  poor: { dot: 'gb-dot--error', label: '差' },
  lost: { dot: 'gb-dot--error', label: '超时' }
}

function latencyDot(row: any): string {
  return LEVEL_TEXT[latencyLevel(row)].dot
}

function latencyQuality(row: any): string {
  return LEVEL_TEXT[latencyLevel(row)].label
}

function latencyText(row: any): string {
  const l = latencyOf(row)
  const level = latencyLevel(row)
  if (level === 'unknown') return '-- ms'
  // 探针超时：显示"上次成功的值"，并且文案由 latencyQuality 说清是超时。
  const v = level === 'lost' ? l?.lastMs : (l?.rttMs ?? l?.lastMs)
  return v === null || v === undefined ? '-- ms' : `${v}ms`
}

/** 悬停明细：均值/极值/丢包/采样次数 —— 单看一个数字看不出抖动。 */
function latencyTitle(row: any): string {
  const l = latencyOf(row)
  if (!l || l.samples === 0) {
    return `尚未测到延迟（每 ${probeIntervalSecs.value}s 探测一次）`
  }
  const parts: string[] = []
  if (l.rttMs !== null && l.rttMs !== undefined) parts.push(`平均 ${l.rttMs}ms`)
  if (l.minMs !== null && l.minMs !== undefined) parts.push(`最小 ${l.minMs}ms`)
  if (l.maxMs !== null && l.maxMs !== undefined) parts.push(`最大 ${l.maxMs}ms`)
  if (l.lastMs !== null && l.lastMs !== undefined) parts.push(`最近一次成功 ${l.lastMs}ms`)
  parts.push(`丢包 ${l.lossPct}%`)
  parts.push(`采样 ${l.samples} 次`)
  if (l.measuredAt) {
    parts.push(`更新于 ${new Date(l.measuredAt * 1000).toLocaleTimeString()}`)
  }
  parts.push(`每 ${probeIntervalSecs.value}s 探测一次`)
  const head = l.ok ? '平台 ↔ 设备 SIP 往返延迟' : `探针超时（连续 ${l.failStreak} 次无响应）`
  return `${head}：${parts.join(' · ')}`
}

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
    // 行换了就立刻拉一次延迟，不等 5s 轮询 —— 否则翻页后新行会先空一下。
    void loadLatency()
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
  // loadData() 内部会顺带拉一次延迟，这里只负责起轮询。
  await loadData()
  latencyTimer = window.setInterval(loadLatency, LATENCY_POLL_MS)
  queryDevices({ page: 1, count: 200 })
    .then((r) => (rowsSuggestion.value = r.data?.list ?? []))
    .catch(() => {})
})

onUnmounted(() => {
  if (latencyTimer !== null) {
    window.clearInterval(latencyTimer)
    latencyTimer = null
  }
})
</script>

<style scoped>
.device-page { padding: 16px; }
.table-card { min-height: 400px; overflow-x: auto; }
.pagination { margin-top: 16px; justify-content: flex-end; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }

.text-tertiary { color: var(--text-tertiary); }
.text-xs { font-size: 12px; }

/* 延迟列：圆点 + 数字 + 档位文字，整体居中不换行
   （"测量中"/"-- ms" 在窄列里会折成两行，很难看） */
.latency-cell {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  white-space: nowrap;
  cursor: help;
}
.latency-cell__q { font-size: var(--text-xs); }

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

/* 流模式下拉选项：左侧模式名，右侧灰色说明（"设备连接平台" 之类），
   让管理员不用去翻协议文档就能选对主动/被动。
   注意：下拉面板被 teleport 到 body，scoped 选择器命中不到，
   所以这段放在下面的全局样式块里（用 popper-class 命名空间隔离）。 */
</style>

<style lang="scss">
.stream-mode-select .el-select-dropdown__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.stream-mode-select .mode-hint {
  margin-left: 16px;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
</style>