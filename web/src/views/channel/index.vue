<template>
  <div class="channel-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">通道列表</h1>
        <p class="page-subtitle">GB/T 28181 通道 · 来自在线国标设备</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button type="primary" :icon="Plus" @click="onAdd">新增通道</el-button>
      </div>
    </div>

    <el-card class="filter-card">
      <el-form :inline="true" :model="query" @submit.prevent="loadData">
        <el-form-item label="关键字">
          <el-input v-model="query.query" placeholder="国标ID / 名称" clearable @keyup.enter="loadData" />
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
      </el-form>
    </el-card>

    <el-card class="table-card">
      <el-table :data="rows" v-loading="loading" stripe border>
        <el-table-column type="index" label="#" width="50" />
        <el-table-column prop="channelId" label="通道国标ID" min-width="200">
          <template #default="{ row }">
            <span class="mono">{{ row.channelId }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="name" label="通道名称" min-width="160" />
        <el-table-column prop="deviceId" label="所属设备" min-width="180">
          <template #default="{ row }">
            <span class="mono">{{ row.deviceId }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="manufacturer" label="厂家" width="120" />
        <el-table-column prop="model" label="型号" width="120" />
        <el-table-column label="状态" width="80">
          <template #default="{ row }">
            <el-tag :type="row.status === 'ON' ? 'success' : 'info'" size="small">
              {{ row.status === 'ON' ? '在线' : '离线' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="civilCode" label="行政区划" width="120" />
        <el-table-column prop="address" label="安装地址" min-width="200" show-overflow-tooltip />
        <el-table-column prop="streamIdentification" label="码流" width="80">
          <template #default="{ row }">
            {{ row.streamIdentification === '0' ? '主码流' : row.streamIdentification === '1' ? '子码流' : '-' }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="220" fixed="right">
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
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { getChannelList, getIndustryList, getTypeList, getNetworkIdentificationList, deleteChannel } from '@/api/channel'
import { startPlay, playSnap } from '@/api/live'
import { useRouter } from 'vue-router'
import Pagination from '@/components/Pagination/index.vue'
import ChannelEditDialog from './EditDialog.vue'

function onPageChange(page: number, size: number) {
  query.page = page
  query.count = size
  loadData()
}

const router = useRouter()
const loading = ref(false)
const rows = ref<any[]>([])
const total = ref(0)
const industryList = ref<string[]>([])
const typeList = ref<string[]>([])
const networkList = ref<string[]>([])
const editVisible = ref(false)
const currentRow = ref<any>({})

const query = reactive({
  page: 1,
  count: 20,
  query: '',
  online: undefined as boolean | undefined,
  channelType: undefined as number | undefined
})

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
}

function resetQuery() {
  query.query = ''
  query.online = undefined
  query.channelType = undefined
  query.page = 1
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

async function onPlay(row: any) {
  try {
    await startPlay(row.deviceId, row.channelId)
    ElMessage.success('播放请求已发送')
    router.push({ name: 'Live', query: { deviceId: row.deviceId, channelId: row.channelId } })
  } catch (e: any) {
    ElMessage.error(e?.message ?? '播放失败')
  }
}

async function onSnapshot(row: any) {
  try {
    const res = await playSnap(row.deviceId, row.channelId)
    ElMessage.success(`抓图已保存: ${res.data?.snapUrl ?? ''}`)
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
    getIndustryList().then((r) => (industryList.value = (r.data as string[]) ?? [])).catch(() => {}),
    getTypeList().then((r) => (typeList.value = (r.data as string[]) ?? [])).catch(() => {}),
    getNetworkIdentificationList().then((r) => (networkList.value = (r.data as string[]) ?? [])).catch(() => {})
  ])
})
</script>

<style scoped>
.channel-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 16px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.filter-card { margin-bottom: 12px; }
.table-card { min-height: 400px; }
.pagination { margin-top: 16px; justify-content: flex-end; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
