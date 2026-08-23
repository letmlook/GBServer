<template>
  <div class="platform-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">上级 / 下级平台</h1>
        <p class="page-subtitle">GB/T 28181 级联 · 通道推送</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button type="primary" :icon="Plus" @click="onAdd">新增平台</el-button>
      </div>
    </div>

    <el-card>
      <el-table :data="rows" v-loading="loading" stripe border>
        <el-table-column prop="name" label="平台名称" min-width="160" />
        <el-table-column prop="serverGbId" label="国标ID" min-width="200">
          <template #default="{ row }"><span class="mono">{{ row.serverGbId }}</span></template>
        </el-table-column>
        <el-table-column prop="serverIp" label="IP" min-width="120">
          <template #default="{ row }"><span class="mono">{{ row.serverIp }}</span></template>
        </el-table-column>
        <el-table-column prop="serverPort" label="端口" width="80" />
        <el-table-column prop="transport" label="传输" width="80" />
        <el-table-column label="在线" width="80">
          <template #default="{ row }">
            <el-tag :type="row.status ? 'success' : 'info'" size="small">{{ row.status ? '在线' : '离线' }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="expires" label="注册有效期" width="100">
          <template #default="{ row }">{{ row.expires ?? '-' }} s</template>
        </el-table-column>
        <el-table-column prop="heartBeatInterval" label="心跳" width="80">
          <template #default="{ row }">{{ row.heartBeatInterval ?? '-' }} s</template>
        </el-table-column>
        <el-table-column label="操作" width="220" fixed="right">
          <template #default="{ row }">
            <el-button link type="primary" @click="onEdit(row)">编辑</el-button>
            <el-button link type="warning" @click="onExit(row)">注销</el-button>
            <el-button link type="danger" @click="onDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <platform-edit-dialog v-model="editVisible" :platform="currentRow" @saved="loadData" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { getPlatformList, deletePlatform, platformExit, type Platform } from '@/api/platform'
import PlatformEditDialog from './EditDialog.vue'

const loading = ref(false)
const rows = ref<any[]>([])
const editVisible = ref(false)
const currentRow = ref<Partial<Platform>>({})

async function loadData() {
  loading.value = true
  try {
    const res = await getPlatformList({ page: 1, count: 200 })
    rows.value = res.data?.list ?? []
  } finally {
    loading.value = false
  }
}

function onAdd() {
  currentRow.value = { transport: 'UDP', registerInterval: 60, heartBeatInterval: 60, heartBeatCount: 3, expires: 3600 }
  editVisible.value = true
}

function onEdit(row: any) {
  currentRow.value = { ...row }
  editVisible.value = true
}

async function onExit(row: any) {
  if (!row.serverGbId) return
  await ElMessageBox.confirm(`确认向 ${row.serverGbId} 发送注销？`, '确认', { type: 'warning' })
  await platformExit(row.serverGbId)
  ElMessage.success('注销请求已发送')
}

async function onDelete(row: any) {
  await ElMessageBox.confirm(`确认删除平台 ${row.name ?? row.serverGbId} ？`, '确认', { type: 'warning' })
  await deletePlatform(row.id ?? 0)
  ElMessage.success('已删除')
  loadData()
}

onMounted(loadData)
</script>

<style scoped>
.platform-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
