<template>
  <div class="media-server-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">媒体节点</h1>
        <p class="page-subtitle">ZLMediaKit 集群 · {{ onlineCount }} 个在线 / {{ rows.length }} 个总计</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button type="primary" :icon="Plus" @click="onAdd">新增节点</el-button>
      </div>
    </div>

    <el-card>
      <el-table :data="rows" v-loading="loading" stripe border>
        <el-table-column prop="id" label="节点 ID" min-width="200">
          <template #default="{ row }"><span class="mono">{{ row.id }}</span></template>
        </el-table-column>
        <el-table-column prop="ip" label="IP" min-width="120">
          <template #default="{ row }"><span class="mono">{{ row.ip }}</span></template>
        </el-table-column>
        <el-table-column prop="httpPort" label="HTTP 端口" width="120" />
        <el-table-column prop="rtmpPort" label="RTMP" width="80" />
        <el-table-column prop="rtspPort" label="RTSP" width="80" />
        <el-table-column label="在线" width="80">
          <template #default="{ row }">
            <el-tag :type="row.status ? 'success' : 'info'" size="small">{{ row.status ? '在线' : '离线' }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="lastKeepaliveTime" label="心跳时间" min-width="180">
          <template #default="{ row }"><span class="mono">{{ row.lastKeepaliveTime ?? '-' }}</span></template>
        </el-table-column>
        <el-table-column label="操作" width="200" fixed="right">
          <template #default="{ row }">
            <el-button link type="primary" @click="onCheck(row)">检测</el-button>
            <el-button link type="primary" @click="onEdit(row)">编辑</el-button>
            <el-button link type="danger" @click="onDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <media-server-edit-dialog v-model="editVisible" :server="currentRow" @saved="loadData" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { getMediaServerList, checkMediaServer, deleteMediaServer, type MediaServer } from '@/api/mediaServer'
import MediaServerEditDialog from './EditDialog.vue'

const loading = ref(false)
const rows = ref<any[]>([])
const editVisible = ref(false)
const currentRow = ref<MediaServer>({} as MediaServer)

const onlineCount = ref(0)

async function loadData() {
  loading.value = true
  try {
    const res = await getMediaServerList()
    rows.value = (res.data as MediaServer[]) ?? []
    onlineCount.value = rows.value.filter((r) => r.status).length
  } finally {
    loading.value = false
  }
}

function onAdd() {
  currentRow.value = {} as MediaServer
  editVisible.value = true
}

function onEdit(row: any) {
  currentRow.value = { ...row }
  editVisible.value = true
}

async function onCheck(row: any) {
  if (!row.id) return
  const res = await checkMediaServer(row.id)
  if ((res.data as any)?.code === 0) {
    ElMessage.success('连通正常')
  } else {
    ElMessage.error(`检测失败: ${(res.data as any)?.msg ?? ''}`)
  }
  loadData()
}

async function onDelete(row: any) {
  await ElMessageBox.confirm(`确认删除媒体节点 ${row.id} ？`, '确认', { type: 'warning' })
  await deleteMediaServer(row.id ?? '')
  ElMessage.success('已删除')
  loadData()
}

onMounted(loadData)
</script>

<style scoped>
.media-server-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
