<template>
  <div class="device-page">
    <div v-if="!currentDeviceId">
      <div class="page-header">
        <div>
          <h1 class="page-title">国标设备</h1>
          <p class="page-subtitle">GB/T 28181 设备注册 · 心跳保活 · 目录同步</p>
        </div>
        <div class="page-actions">
          <el-button @click="loadData">刷新</el-button>
          <el-button :icon="Plus" type="primary" @click="onAdd">新增设备</el-button>
        </div>
      </div>

      <GbSearchForm :model="query" @search="loadData" @reset="resetQuery">
        <template #default="{ model }">
          <el-form-item label="关键字">
            <el-input :model-value="(model as any).query" @update:model-value="(v: string) => (model as any).query = v" placeholder="国标ID / 名称 / IP" clearable @keyup.enter="loadData" />
          </el-form-item>
          <el-form-item label="状态">
            <el-select :model-value="(model as any).status" @update:model-value="(v: string) => (model as any).status = v" placeholder="全部" clearable style="width: 140px">
              <el-option label="在线" value="ON" />
              <el-option label="离线" value="OFF" />
              <el-option label="全部" value="" />
            </el-select>
          </el-form-item>
        </template>
      </GbSearchForm>

      <el-card class="table-card">
        <el-table :data="rows" v-loading="loading" stripe border>
          <el-table-column type="index" label="#" width="50" />
          <el-table-column prop="deviceId" label="国标ID" min-width="200">
            <template #default="{ row }">
              <el-link type="primary" :underline="false" @click="showChannels(row)">
                <span class="mono">{{ row.deviceId }}</span>
              </el-link>
            </template>
          </el-table-column>
          <el-table-column prop="name" label="名称" min-width="140" />
          <el-table-column prop="manufacturer" label="厂家" width="100" />
          <el-table-column prop="model" label="型号" width="100" />
          <el-table-column prop="ip" label="IP" width="120">
            <template #default="{ row }"><span class="mono">{{ row.ip }}</span></template>
          </el-table-column>
          <el-table-column prop="transport" label="信令" width="80" />
          <el-table-column prop="streamMode" label="流模式" width="100" />
          <el-table-column label="在线" width="80">
            <template #default="{ row }">
              <el-tag :type="isOnline(row) ? 'success' : 'info'" size="small">
                {{ isOnline(row) ? '在线' : '离线' }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="channelCount" label="通道数" width="80" />
          <el-table-column label="操作" width="280" fixed="right">
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
      <el-page-header @back="currentDeviceId = ''">
        <template #content>
          <span class="page-title">通道 · {{ currentDeviceId }}</span>
        </template>
      </el-page-header>
      <el-card class="table-card" style="margin-top: 12px">
        <el-table :data="channels" v-loading="channelLoading" stripe border>
          <el-table-column type="index" label="#" width="50" />
          <el-table-column prop="channelId" label="通道ID" min-width="200">
            <template #default="{ row }"><span class="mono">{{ row.channelId }}</span></template>
          </el-table-column>
          <el-table-column prop="name" label="通道名称" min-width="160" />
          <el-table-column prop="manufacturer" label="厂家" width="100" />
          <el-table-column label="在线" width="80">
            <template #default="{ row }">
              <el-tag :type="row.status === 'ON' ? 'success' : 'info'" size="small">
                {{ row.status === 'ON' ? '在线' : '离线' }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="civilCode" label="行政区划" width="100" />
          <el-table-column prop="address" label="地址" min-width="200" show-overflow-tooltip />
          <el-table-column prop="subCount" label="子通道" width="80" />
        </el-table>
      </el-card>
    </div>

    <device-edit-dialog v-model="editVisible" :device="currentRow" @saved="loadData" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { queryDevices, deleteDevice, sync, setGuard, resetGuard, queryChannels, type DeviceRecord } from '@/api/device'
import GbSearchForm from '@/components/GbSearchForm/index.vue'
import DeviceEditDialog from './EditDialog.vue'

const loading = ref(false)
const channelLoading = ref(false)
const rows = ref<any[]>([])
const channels = ref<any[]>([])
const total = ref(0)
const currentDeviceId = ref('')
const currentRow = ref<any>({})
const editVisible = ref(false)

const query = reactive({
  page: 1,
  count: 20,
  query: '',
  status: ''
})

function isOnline(row: any): boolean {
  return row.online === true || row.online === 1 || row.status === 'ON'
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

async function showChannels(row: any) {
  currentDeviceId.value = row.deviceId ?? ''
  channelLoading.value = true
  try {
    const res = await queryChannels(row.deviceId ?? '', { page: 1, count: 500 })
    channels.value = res.data?.list ?? []
  } catch {
    channels.value = []
  } finally {
    channelLoading.value = false
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

onMounted(loadData)
</script>

<style scoped>
.device-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 16px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.filter-card { margin-bottom: 12px; }
.table-card { min-height: 400px; }
.pagination { margin-top: 16px; justify-content: flex-end; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
