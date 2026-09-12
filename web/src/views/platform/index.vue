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
        <el-table-column prop="name" label="平台名称" min-width="150" show-overflow-tooltip />
        <el-table-column label="国标ID" min-width="200">
          <template #default="{ row }"><span class="mono">{{ row.serverGBId }}</span></template>
        </el-table-column>
        <el-table-column label="国标域" min-width="130">
          <template #default="{ row }"><span class="mono">{{ row.serverGBDomain || '-' }}</span></template>
        </el-table-column>
        <el-table-column label="IP" min-width="120">
          <template #default="{ row }"><span class="mono">{{ row.serverIp }}</span></template>
        </el-table-column>
        <el-table-column prop="serverPort" label="端口" width="80" />
        <el-table-column prop="transport" label="传输" width="80" />
        <el-table-column label="通道数" width="80">
          <template #default="{ row }">{{ row.channelCount ?? 0 }}</template>
        </el-table-column>
        <el-table-column label="启用" width="80">
          <template #default="{ row }">
            <el-tag :type="row.enable ? 'success' : 'info'" size="small">
              {{ row.enable ? '已启用' : '未启用' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="在线" width="80">
          <template #default="{ row }">
            <el-tag :type="row.status ? 'success' : 'info'" size="small">
              {{ row.status ? '在线' : '离线' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="注册周期" width="100">
          <template #default="{ row }">{{ row.expires ?? '-' }} s</template>
        </el-table-column>
        <el-table-column label="心跳周期" width="100">
          <template #default="{ row }">{{ row.keepTimeout ?? '-' }} s</template>
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
const rows = ref<Platform[]>([])
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
  currentRow.value = { transport: 'UDP', expires: 3600, keepTimeout: 60, enable: true }
  editVisible.value = true
}

function onEdit(row: any) {
  currentRow.value = { ...row }
  editVisible.value = true
}

async function onExit(row: any) {
  // 后端按 serverGBId 定位并真的发 Expires:0 注销报文（此前键名写错 → 静默 return）
  if (!row.serverGBId) {
    ElMessage.warning('该平台缺少国标ID，无法注销')
    return
  }
  await ElMessageBox.confirm(`确认向 ${row.serverGBId} 发送注销？该平台将被置为停用。`, '确认', {
    type: 'warning'
  })
  const res = await platformExit(row.serverGBId)
  const warning = (res.data as any)?.sipWarning
  if (warning) ElMessage.warning(`注销报文发送失败：${warning}`)
  else ElMessage.success('注销请求已发送')
  loadData()
}

async function onDelete(row: any) {
  await ElMessageBox.confirm(`确认删除平台 ${row.name ?? row.serverGBId} ？`, '确认', { type: 'warning' })
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
