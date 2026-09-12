<template>
  <div class="stream-proxy-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">拉流代理</h1>
        <p class="page-subtitle">RTSP / RTMP / HLS 拉流转 ZLMediaKit · 可转 GB28181 通道</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button type="primary" :icon="Plus" @click="onAdd">新增代理</el-button>
      </div>
    </div>

    <el-card>
      <el-form :inline="true" class="filter-bar">
        <el-form-item label="关键字">
          <el-input
            v-model="filter.query"
            placeholder="名称 / App / Stream / 源地址"
            clearable
            style="width: 240px"
            @keyup.enter="onSearch"
            @clear="onSearch"
          />
        </el-form-item>
        <el-form-item label="拉流状态">
          <el-select v-model="filter.pulling" placeholder="全部" clearable style="width: 140px" @change="onSearch">
            <el-option label="全部" value="" />
            <el-option label="正在拉流" value="true" />
            <el-option label="尚未拉流" value="false" />
          </el-select>
        </el-form-item>
        <el-form-item label="流媒体">
          <el-select v-model="filter.mediaServerId" placeholder="全部" clearable style="width: 180px" @change="onSearch">
            <el-option label="全部" value="" />
            <el-option v-for="ms in mediaServers" :key="ms.id" :label="ms.id ?? ''" :value="ms.id ?? ''" />
          </el-select>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="onSearch">查询</el-button>
        </el-form-item>
      </el-form>

      <el-table :data="rows" v-loading="loading" stripe border>
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="name" label="名称" min-width="140" show-overflow-tooltip />
        <el-table-column label="代理方式" width="110">
          <template #default="{ row }">
            <el-tag size="small" :type="row.type === 'ffmpeg' ? 'warning' : 'info'">
              {{ row.type === 'ffmpeg' ? 'FFmpeg' : '默认' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="app" label="App" min-width="90" />
        <el-table-column prop="stream" label="Stream" min-width="140">
          <template #default="{ row }"><span class="mono">{{ row.stream }}</span></template>
        </el-table-column>
        <el-table-column label="源地址" min-width="240" show-overflow-tooltip>
          <template #default="{ row }"><span class="mono">{{ row.srcUrl }}</span></template>
        </el-table-column>
        <el-table-column label="媒体节点" min-width="140">
          <template #default="{ row }">{{ row.mediaServerId || '自动' }}</template>
        </el-table-column>
        <el-table-column label="拉流状态" width="100">
          <template #default="{ row }">
            <el-tag :type="row.pulling ? 'success' : 'info'" size="small">
              {{ row.pulling ? '正在拉流' : '尚未拉流' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="启用" width="90">
          <template #default="{ row }">
            <el-tag :type="row.enable ? 'success' : 'info'" size="small">
              {{ row.enable ? '已启用' : '未启用' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="createTime" label="创建时间" width="170" />
        <el-table-column label="操作" width="220" fixed="right">
          <template #default="{ row }">
            <el-button link type="success" :disabled="!!row.pulling" @click="onStart(row)">播放</el-button>
            <el-button link type="warning" :disabled="!row.pulling" @click="onStop(row)">停止</el-button>
            <el-button link type="primary" @click="onEdit(row)">编辑</el-button>
            <el-button link type="danger" @click="onDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>

      <el-pagination
        class="pager"
        layout="total, sizes, prev, pager, next"
        :total="total"
        :current-page="page"
        :page-size="count"
        :page-sizes="[10, 20, 50, 100]"
        @current-change="onPageChange"
        @size-change="onSizeChange"
      />
    </el-card>

    <stream-proxy-edit-dialog v-model="editVisible" :proxy="currentRow" @saved="loadData" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  getStreamProxyList,
  startStreamProxy,
  stopStreamProxy,
  deleteStreamProxy,
  type StreamProxy
} from '@/api/streamProxy'
import { getMediaServerOnlineList, type MediaServer } from '@/api/mediaServer'
import StreamProxyEditDialog from './EditDialog.vue'

const loading = ref(false)
const rows = ref<StreamProxy[]>([])
const total = ref(0)
const page = ref(1)
const count = ref(20)
const editVisible = ref(false)
const currentRow = ref<Partial<StreamProxy>>({})
const mediaServers = ref<MediaServer[]>([])

// 后端支持 query / pulling / mediaServerId 三个筛选（此前 query 收了不用）
const filter = reactive<{ query: string; pulling: string; mediaServerId: string }>({
  query: '',
  pulling: '',
  mediaServerId: ''
})

async function loadData() {
  loading.value = true
  try {
    const res = await getStreamProxyList({
      page: page.value,
      count: count.value,
      query: filter.query || undefined,
      pulling: filter.pulling === '' ? undefined : filter.pulling,
      mediaServerId: filter.mediaServerId || undefined
    })
    rows.value = res.data?.list ?? []
    total.value = res.data?.total ?? 0
  } finally {
    loading.value = false
  }
}

function onSearch() {
  page.value = 1
  loadData()
}

function onPageChange(p: number) {
  page.value = p
  loadData()
}

function onSizeChange(s: number) {
  count.value = s
  page.value = 1
  loadData()
}

async function loadMediaServers() {
  try {
    const res = await getMediaServerOnlineList()
    mediaServers.value = res.data ?? []
  } catch {
    mediaServers.value = []
  }
}

function onAdd() {
  currentRow.value = { type: 'default', enable: true }
  editVisible.value = true
}

function onEdit(row: any) {
  currentRow.value = { ...row }
  editVisible.value = true
}

async function onStart(row: any) {
  await startStreamProxy(row.id ?? 0)
  ElMessage.success('拉流代理已启动')
  loadData()
}

async function onStop(row: any) {
  await stopStreamProxy(row.id ?? 0)
  ElMessage.success('拉流代理已停止')
  loadData()
}

async function onDelete(row: any) {
  await ElMessageBox.confirm(`确认删除代理 ${row.name || row.stream} ？`, '确认', { type: 'warning' })
  await deleteStreamProxy(row.id ?? 0)
  ElMessage.success('已删除')
  loadData()
}

onMounted(() => {
  loadMediaServers()
  loadData()
})
</script>

<style scoped>
.stream-proxy-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.filter-bar { margin-bottom: 4px; }
.pager { margin-top: 12px; justify-content: flex-end; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
