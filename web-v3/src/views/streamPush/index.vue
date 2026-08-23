<template>
  <div class="stream-push-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">推流列表</h1>
        <p class="page-subtitle">RTSP/RTMP/HLS 推流 → ZLMediaKit / 国标</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button type="danger" :disabled="!selection.length" @click="onBatchRemove">批量删除</el-button>
        <el-button type="primary" :icon="Plus" @click="onAdd">新增推流</el-button>
      </div>
    </div>

    <el-card>
      <el-table :data="rows" v-loading="loading" stripe border @selection-change="onSelection">
        <el-table-column type="selection" width="48" />
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="app" label="App" min-width="100" />
        <el-table-column prop="stream" label="Stream" min-width="160">
          <template #default="{ row }"><span class="mono">{{ row.stream }}</span></template>
        </el-table-column>
        <el-table-column prop="url" label="源 URL" min-width="280" show-overflow-tooltip>
          <template #default="{ row }"><span class="mono">{{ row.url }}</span></template>
        </el-table-column>
        <el-table-column prop="mediaServerId" label="媒体节点" min-width="140" />
        <el-table-column label="状态" width="100">
          <template #default="{ row }">
            <el-tag :type="row.status === 1 ? 'success' : 'info'" size="small">
              {{ row.status === 1 ? '推送中' : '停止' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="320" fixed="right">
          <template #default="{ row }">
            <el-button link type="success" :disabled="row.status === 1" @click="onStart(row)">启动</el-button>
            <el-button link type="warning" :disabled="row.status !== 1" @click="onStop(row)">停止</el-button>
            <el-button link type="primary" @click="onEdit(row)">编辑</el-button>
            <el-button link type="danger" @click="onRemove(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <stream-push-edit-dialog v-model="editVisible" :push="currentRow" @saved="loadData" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  getStreamPushList,
  deleteStreamPush,
  batchDeleteStreamPush,
  startStreamPush,
  stopStreamPush,
  type StreamPush
} from '@/api/streamPush'
import StreamPushEditDialog from './EditDialog.vue'

const loading = ref(false)
const rows = ref<any[]>([])
const editVisible = ref(false)
const currentRow = ref<Partial<StreamPush>>({})
const selection = ref<StreamPush[]>([])

async function loadData() {
  loading.value = true
  try {
    const res = await getStreamPushList({ page: 1, count: 200 })
    rows.value = res.data?.list ?? []
  } finally {
    loading.value = false
  }
}

function onAdd() {
  currentRow.value = {}
  editVisible.value = true
}

function onEdit(row: any) {
  currentRow.value = { ...row }
  editVisible.value = true
}

async function onStart(row: any) {
  await startStreamPush(row.id ?? 0)
  ElMessage.success('启动指令已发送')
  setTimeout(loadData, 1000)
}

async function onStop(row: any) {
  await stopStreamPush(row.id ?? 0)
  ElMessage.success('停止指令已发送')
  setTimeout(loadData, 1000)
}

async function onRemove(row: any) {
  await ElMessageBox.confirm(`确认删除推流 ${row.app}/${row.stream} ？`, '确认', { type: 'warning' })
  await deleteStreamPush(row.id ?? 0)
  ElMessage.success('已删除')
  loadData()
}

async function onBatchRemove() {
  await ElMessageBox.confirm(`确认批量删除 ${selection.value.length} 条？`, '确认', { type: 'warning' })
  await batchDeleteStreamPush(selection.value.map((r) => r.id ?? 0))
  ElMessage.success('已批量删除')
  loadData()
}

function onSelection(arr: StreamPush[]) {
  selection.value = arr
}

onMounted(loadData)
</script>

<style scoped>
.stream-push-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
