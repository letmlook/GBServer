<template>
  <div class="channel-page">
    <el-card class="filter-card">
      <el-form class="gb-query-row" :inline="true" :model="query" @submit.prevent="loadData">
        <el-form-item label="关键字">
          <el-autocomplete
            v-model="query.query"
            :fetch-suggestions="searchSuggest"
            placeholder="国标ID / 通道名 / 设备ID（输即搜）"
            clearable
            style="width: 240px"
            @keyup.enter="loadData"
            @select="onSelectSuggest"
          />
        </el-form-item>
        <el-form-item label="状态">
          <el-select v-model="query.online" placeholder="全部" clearable style="width: 120px">
            <el-option label="在线" :value="true" />
            <el-option label="离线" :value="false" />
          </el-select>
        </el-form-item>
        <el-form-item label="类型">
          <el-select v-model="query.channelType" placeholder="全部" clearable style="width: 120px">
            <el-option label="设备" :value="0" />
            <el-option label="目录" :value="1" />
          </el-select>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="loadData">查询</el-button>
          <el-button @click="resetQuery">重置</el-button>
        </el-form-item>
        <el-form-item class="gb-query-actions">
          <el-button @click="loadData">刷新</el-button>
          <el-button type="primary" :icon="Plus" @click="onAdd">新增通道</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card class="table-card">
      <el-table :data="rows" v-loading="loading" stripe border>
        <el-table-column label="缩略图" width="120" align="center">
          <template #default="{ row }">
            <div class="thumb-cell" :class="{ 'thumb-cell--clickable': !!row.thumb }">
              <!-- 用 el-image 而不是裸 <img>：点开就是内置大图预览器，
                   带缩放 / 旋转 / ESC 关闭，无需自己写 lightbox。
                   preview-teleported 让预览层挂到 body —— 否则会被
                   表格容器的 overflow 裁掉。 -->
              <el-image
                v-if="row.thumb && !failedThumbs.has(row.channelId)"
                :src="row.thumb"
                :preview-src-list="[row.thumb]"
                :initial-index="0"
                fit="cover"
                preview-teleported
                hide-on-click-modal
                :alt="row.name ?? row.channelId"
                class="thumb-cell__img"
                @error="onThumbError(row)"
              />
              <div v-else class="thumb-cell__placeholder">
                <el-icon :size="20"><VideoCameraFilled /></el-icon>
              </div>
            </div>
          </template>
        </el-table-column>
        <el-table-column type="index" label="#" width="50" />
        <el-table-column prop="channelId" label="通道国标ID" min-width="180">
          <template #default="{ row }">
            <span class="mono">{{ row.channelId }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="name" label="通道名称" min-width="160" show-overflow-tooltip />
        <el-table-column prop="deviceId" label="所属设备" min-width="160">
          <template #default="{ row }">
            <span class="mono">{{ row.deviceId }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="manufacturer" label="厂家" width="100" show-overflow-tooltip />
        <el-table-column prop="model" label="型号" width="100" show-overflow-tooltip />
        <el-table-column label="状态" width="80">
          <template #default="{ row }">
            <el-tag :type="row.status === 'ON' ? 'success' : 'info'" size="small">
              {{ row.status === 'ON' ? '在线' : '离线' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="civilCode" label="行政区划" width="100" />
        <el-table-column label="安装地址" min-width="200" show-overflow-tooltip>
          <template #default="{ row }">
            {{ formatAddress(row.address) }}
          </template>
        </el-table-column>
        <el-table-column label="码流" width="90">
          <template #default="{ row }">
            <el-tag
              v-if="row.streamIdentification === '0'"
              type="success"
              size="small"
            >主码流</el-tag>
            <el-tag
              v-else-if="row.streamIdentification === '1'"
              type="warning"
              size="small"
            >子码流</el-tag>
            <span v-else class="text-tertiary">-</span>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="240" fixed="right">
          <template #default="{ row }">
            <el-button link type="primary" @click="onEdit(row)">编辑</el-button>
            <el-button link type="primary" @click="onPlay(row)">播放</el-button>
            <el-button link type="primary" @click="onSnapshot(row)">抓图</el-button>
            <el-button link type="danger" @click="onDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>

      <pagination
        :page="query.page"
        :size="query.count"
        :total="total"
        @change="onPageChange"
      />
    </el-card>

    <channel-edit-dialog
      v-model="editVisible"
      :channel="currentRow"
      :industry-list="industryList"
      :type-list="typeList"
      :network-list="networkList"
      @saved="loadData"
    />

    <!-- 抓图预览：成功弹图 -->
    <SnapPreview v-model="snapVisible" :items="snapItems" @clear="snapItems = []" />

    <!-- 通道播放对话框：取代跳转 /live -->
    <ChannelPlayDialog
      v-model="playVisible"
      :channel="playingChannel"
      @snap="onPlaySnap"
    />
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { Plus, VideoCameraFilled } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  getChannelList,
  getIndustryList,
  getTypeList,
  getNetworkIdentificationList,
  deleteChannel,
  type ChannelCodeType
} from '@/api/channel'
import { captureSnap, listSnapshots, snapshotKey } from '@/api/live'
import Pagination from '@/components/Pagination/index.vue'
import ChannelEditDialog from './EditDialog.vue'
import SnapPreview from '@/components/SnapPreview/index.vue'
import ChannelPlayDialog from '@/components/ChannelPlayDialog/index.vue'

function onPageChange(page: number, size: number) {
  query.page = page
  query.count = size
  loadData()
}
const loading = ref(false)
const rows = ref<any[]>([])
const total = ref(0)
// 这三个接口返回的是 `{name, code}`（WVP `IndustryCodeType`/`DeviceType`/
// `NetworkIdentificationType`），不是字符串数组 —— 早期按 string[] 用，
// 下拉里显示的是 "[object Object]"。
const industryList = ref<ChannelCodeType[]>([])
const typeList = ref<ChannelCodeType[]>([])
const networkList = ref<ChannelCodeType[]>([])
const editVisible = ref(false)
const currentRow = ref<any>({})

// 通道播放对话框状态
const playVisible = ref(false)
const playingChannel = ref<{ deviceId: string; channelId: string; name: string } | null>(null)

const query = reactive({
  page: 1,
  count: 20,
  query: '',
  online: undefined as boolean | undefined,
  channelType: undefined as number | undefined
})

/**
 * 给当前页的通道铺上**已保存的**缩略图。
 *
 * 缩略图由后端在每次点播时自动抓帧落盘（见 `spawn_snapshot_capture`），
 * 这里只是把存量读回来 —— 一次批量请求搞定整页，不碰 ZLM、不唤醒设备、
 * 不等解码。没存过缩略图的通道保持占位图标，等它被点播过一次自然就有了。
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
      if (url) {
        ch.thumb = url
        // 拿到图就清掉之前的失败标记，让缩略图列重新渲染
        failedThumbs.value.delete(ch.channelId)
      }
    }
  } catch {
    // 静默：全部保持占位图标
  }
}

// 抓图预览：累积所有抓图快照，浮窗可放大
const snapVisible = ref(false)
const snapItems = ref<{ deviceId: string; channelId: string; name: string; snapUrl: string; time: number }[]>([])

/**
 * 缩略图 `<img>` 加载失败（404/403/网络错）的通道集合 ——
 * 失败的行立刻回落到占位图标，而不是显示浏览器默认的"碎图"。
 * 用 channelId 做键，播放/抓图后会清掉这个标记重试。
 */
const failedThumbs = ref<Set<string>>(new Set())

function onThumbError(row: any) {
  const key = row?.channelId
  if (!key) return
  failedThumbs.value.add(key)
  // 清掉 URL，让模板稳定走占位分支（否则 src 不变会反复触发 error）
  row.thumb = ''
}

async function loadData() {
  loading.value = true
  try {
    const res = await getChannelList({
      page: query.page,
      count: query.count,
      query: query.query,
      online: query.online,
      channelType: query.channelType
    })
    rows.value = res.data?.list ?? []
    total.value = res.data?.total ?? 0
  } catch {
    rows.value = []
    total.value = 0
  } finally {
    loading.value = false
  }
  // 列表渲染完后铺上已保存的缩略图（一次批量请求，不阻塞 UI）
  void loadThumbs(rows.value)
}

function resetQuery() {
  query.query = ''
  query.online = undefined
  query.channelType = undefined
  query.page = 1
  loadData()
}

/**
 * 安装地址占位：null/空/未配置 时显示 `-`。
 * 历史数据里部分通道的 address 是 SQL 默认值 `'Address'`（早期
 * 国标注册报文未携带 Address 字段，DB DEFAULT 留了字面量），这里一并
 * 当作"无地址"处理。
 */
function formatAddress(v: unknown): string {
  if (v === null || v === undefined) return '-'
  const s = String(v).trim()
  if (!s) return '-'
  // 后端默认值字面量
  if (s.toLowerCase() === 'address') return '-'
  return s
}

/**
 * 自动下拉建议：
 * - 不打后端搜索接口（接口按全字段 LIKE 匹配，敲一个字就返 1000 行很重）
 * - 直接用本地已加载的 rows 做"前端过滤 + 联想"。
 * - 这意味着：第一次 query 后才有联想；为了一开始就有联想，进入页面时
 *   静默拉一次 count=200 写入 rowsSuggestion（见 onMounted）。
 */
const rowsSuggestion = ref<any[]>([])

function searchSuggest(kw: string, cb: (arr: any[]) => void) {
  const k = (kw ?? '').trim().toLowerCase()
  if (!k) {
    cb(rowsSuggestion.value.slice(0, 30))
    return
  }
  const matched = rowsSuggestion.value
    .filter(
      (r) =>
        String(r.channelId ?? '').toLowerCase().includes(k) ||
        String(r.deviceId ?? '').toLowerCase().includes(k) ||
        String(r.name ?? '').toLowerCase().includes(k)
    )
    .slice(0, 30)
    .map((r) => ({ value: formatSuggestLabel(r) }))
  cb(matched)
}

function formatSuggestLabel(r: any): string {
  return `${r.channelId} · ${r.name ?? '-'} (${r.deviceId ?? '-'})`
}

function onSelectSuggest(item: any) {
  // 选中下拉项时把里面解出的 channelId 写回 input（el-autocomplete 默认会把 label 写回）
  // 加载一次以确保过滤生效
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

function onPlay(row: any) {
  playingChannel.value = {
    deviceId: row.deviceId,
    channelId: row.channelId,
    name: row.name ?? row.channelId
  }
  playVisible.value = true
}

/**
 * 播放对话框回传的抓图。
 *
 * `auto=true` 是"拉起流后后端自动抓的那一帧" —— 只用来填该行的缩略图，
 * **不弹预览窗口**（用户只是点了播放，不该被一个抓图浮窗打断）。
 * 用户在对话框里**手动点「抓图」**时才把图推进预览浮窗展示。
 */
function onPlaySnap(snapUrl: string, payload?: { auto?: boolean }) {
  if (!playingChannel.value || !snapUrl) return
  const { deviceId, channelId, name } = playingChannel.value

  // 同步缩略图到行（自动/手动都要做）。URL 已带版本号，不用再加 cache-busting。
  const row = rows.value.find((r) => r.deviceId === deviceId && r.channelId === channelId)
  if (row) {
    row.thumb = snapUrl
    // 之前加载失败被标记过的行，这次拿到真图就清掉标记重试
    failedThumbs.value.delete(channelId)
  }

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

/**
 * 「抓图」按钮：让后端立刻从该通道当前流里抓一帧、覆盖保存为缩略图，
 * 然后取回新 URL 弹预览窗。
 *
 * 该通道当前**必须有活跃的流**（国标设备按需推流），否则后端会明确报错
 * 提示"要先播放一次" —— 这比返回一张过期图或干等超时更诚实。
 */
async function onSnapshot(row: any) {
  if (!row?.deviceId || !row?.channelId) return
  const key = snapshotKey(row.deviceId, row.channelId)
  try {
    await captureSnap(row.deviceId, row.channelId)
    const res = await listSnapshots([key])
    const snapUrl = res?.data?.[key] ?? ''
    if (snapUrl) {
      row.thumb = snapUrl
      failedThumbs.value.delete(row.channelId)
      snapItems.value.unshift({
        deviceId: row.deviceId,
        channelId: row.channelId,
        name: row.name ?? row.channelId,
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

async function onDelete(row: any) {
  await ElMessageBox.confirm(`确认删除通道 ${row.name ?? row.channelId} ？该操作不可恢复`, '危险操作', {
    type: 'error'
  })
  const id = row.id ?? row.channelId
  if (!id) {
    ElMessage.error('通道缺少主键 id，无法删除')
    return
  }
  await deleteChannel(id)
  ElMessage.success('已删除')
  loadData()
}

onMounted(async () => {
  await Promise.all([
    loadData(),
    // 静默拉一份中等数量通道，写入下拉建议池
    getChannelList({ page: 1, count: 200 })
      .then((r) => (rowsSuggestion.value = r.data?.list ?? []))
      .catch(() => {}),
    getIndustryList().then((r) => (industryList.value = r.data ?? [])).catch(() => {}),
    getTypeList().then((r) => (typeList.value = r.data ?? [])).catch(() => {}),
    getNetworkIdentificationList().then((r) => (networkList.value = r.data ?? [])).catch(() => {})
  ])
})
</script>

<style scoped>
.channel-page { padding: 16px; }
.filter-card { margin-bottom: 12px; }
.table-card { min-height: 400px; overflow-x: auto; }
.pagination { margin-top: 16px; justify-content: flex-end; }

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
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: var(--text-sm); }
</style>