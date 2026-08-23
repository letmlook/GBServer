<template>
  <div class="cloud-record-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">云端录像</h1>
        <p class="page-subtitle">ZLMediaKit MP4 / HLS 录像检索与下载</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button type="success" :disabled="!selection.length" @click="onDownloadZip">打包下载</el-button>
      </div>
    </div>

    <el-card class="filter-card">
      <el-form :inline="true">
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
        <el-form-item label="开始">
          <el-date-picker v-model="query.startTime" type="datetime" />
        </el-form-item>
        <el-form-item label="结束">
          <el-date-picker v-model="query.endTime" type="datetime" />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="loadData">查询</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card>
      <el-table :data="rows" v-loading="loading" stripe border @selection-change="onSelection">
        <el-table-column type="selection" width="48" />
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="app" label="App" width="100" />
        <el-table-column prop="stream" label="Stream" min-width="160">
          <template #default="{ row }"><span class="mono">{{ row.stream }}</span></template>
        </el-table-column>
        <el-table-column prop="startTime" label="开始" min-width="170">
          <template #default="{ row }"><span class="mono">{{ row.startTime }}</span></template>
        </el-table-column>
        <el-table-column prop="endTime" label="结束" min-width="170">
          <template #default="{ row }"><span class="mono">{{ row.endTime }}</span></template>
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
import { onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
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

const query = reactive({
  page: 1,
  count: 20,
  deviceId: '',
  channelId: '',
  app: '',
  stream: '',
  startTime: undefined as Date | undefined,
  endTime: undefined as Date | undefined
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
      startTime: query.startTime?.toISOString(),
      endTime: query.endTime?.toISOString()
    })
    rows.value = res.data?.list ?? []
    total.value = res.data?.total ?? 0
  } finally {
    loading.value = false
  }
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
  const res = await getCloudRecordPlayPath(row.id ?? 0)
  const path = (res.data as any)?.path ?? ''
  if (path) {
    window.open(path, '_blank')
  } else {
    ElMessage.info('请通过 ZLMediaKit URL 直接播放')
  }
}

async function onDownload(row: CloudRecord) {
  ElMessage.info(`请通过 /api/cloud/record/download/zip?ids=${row.id} 下载`)
}

async function onDelete(row: CloudRecord) {
  await ElMessageBox.confirm('确认删除该云端录像？', '确认', { type: 'warning' })
  await deleteCloudRecord(row.id ?? 0)
  ElMessage.success('已删除')
  loadData()
}

async function onDownloadZip() {
  const res = await downloadCloudRecordZip(selection.value.map((r) => r.id ?? 0))
  const url = (res.data as any)?.url
  if (url) window.open(url, '_blank')
}

onMounted(loadData)
</script>

<style scoped>
.cloud-record-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.filter-card { margin-bottom: 12px; }
.pagination { margin-top: 16px; justify-content: flex-end; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
