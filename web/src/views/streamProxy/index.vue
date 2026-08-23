<template>
  <div class="stream-proxy-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">拉流代理</h1>
        <p class="page-subtitle">RTSP / RTMP / HLS 拉流转 GB28181 / ZLMediaKit</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button type="primary" :icon="Plus" @click="onAdd">新增代理</el-button>
      </div>
    </div>

    <el-card>
      <el-table :data="rows" v-loading="loading" stripe border>
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="name" label="名称" min-width="160" />
        <el-table-column prop="type" label="类型" width="100" />
        <el-table-column prop="app" label="App" min-width="100" />
        <el-table-column prop="stream" label="Stream" min-width="160">
          <template #default="{ row }"><span class="mono">{{ row.stream }}</span></template>
        </el-table-column>
        <el-table-column prop="url" label="源 URL" min-width="240" show-overflow-tooltip>
          <template #default="{ row }"><span class="mono">{{ row.url }}</span></template>
        </el-table-column>
        <el-table-column label="启用" width="80">
          <template #default="{ row }">
            <el-switch v-model="row.enabled" @change="onToggle(row)" />
          </template>
        </el-table-column>
        <el-table-column label="状态" width="80">
          <template #default="{ row }">
            <el-tag :type="row.status === 1 ? 'success' : 'info'" size="small">
              {{ row.status === 1 ? '运行中' : '停止' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="280" fixed="right">
          <template #default="{ row }">
            <el-button link type="success" :disabled="row.status === 1" @click="onStart(row)">启动</el-button>
            <el-button link type="warning" :disabled="row.status !== 1" @click="onStop(row)">停止</el-button>
            <el-button link type="primary" @click="onEdit(row)">编辑</el-button>
            <el-button link type="danger" @click="onDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <stream-proxy-edit-dialog v-model="editVisible" :proxy="currentRow" @saved="loadData" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { getStreamProxyList, startStreamProxy, stopStreamProxy, deleteStreamProxy, type StreamProxy } from '@/api/streamProxy'
import StreamProxyEditDialog from './EditDialog.vue'

const loading = ref(false)
const rows = ref<any[]>([])
const editVisible = ref(false)
const currentRow = ref<Partial<StreamProxy>>({})

async function loadData() {
  loading.value = true
  try {
    const res = await getStreamProxyList({ page: 1, count: 200 })
    rows.value = res.data?.list ?? []
  } finally {
    loading.value = false
  }
}

function onAdd() {
  currentRow.value = { type: 'rtsp', enabled: true }
  editVisible.value = true
}

function onEdit(row: any) {
  currentRow.value = { ...row }
  editVisible.value = true
}

async function onStart(row: any) {
  await startStreamProxy(row.id ?? 0)
  ElMessage.success('启动指令已发送')
  setTimeout(loadData, 1000)
}

async function onStop(row: any) {
  await stopStreamProxy(row.id ?? 0)
  ElMessage.success('停止指令已发送')
  setTimeout(loadData, 1000)
}

async function onToggle(row: any) {
  if (row.enabled) await startStreamProxy(row.id ?? 0)
  else await stopStreamProxy(row.id ?? 0)
  setTimeout(loadData, 1000)
}

async function onDelete(row: any) {
  await ElMessageBox.confirm(`确认删除代理 ${row.name} ？`, '确认', { type: 'warning' })
  await deleteStreamProxy(row.id ?? 0)
  ElMessage.success('已删除')
  loadData()
}

onMounted(loadData)
</script>

<style scoped>
.stream-proxy-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
