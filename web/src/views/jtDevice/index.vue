<template>
  <div class="jt-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">JT/T 1078 车载终端</h1>
        <p class="page-subtitle">部标 808/1078 · 终端管理 · 区域/路线围栏</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button type="primary" :icon="Plus" @click="onAdd">新增终端</el-button>
      </div>
    </div>

    <el-tabs v-model="activeTab">
      <el-tab-pane label="终端列表" name="terminal">
        <el-card>
          <el-table :data="terminals" v-loading="loading" stripe border>
            <el-table-column prop="id" label="ID" width="60" />
            <el-table-column prop="phoneNumber" label="手机号" min-width="140">
              <template #default="{ row }"><span class="mono">{{ row.phoneNumber }}</span></template>
            </el-table-column>
            <el-table-column prop="plateNo" label="车牌号" min-width="120" />
            <el-table-column prop="model" label="型号" width="120" />
            <el-table-column prop="manufacturer" label="厂商" width="100" />
            <el-table-column label="在线" width="80">
              <template #default="{ row }">
                <el-tag :type="row.status === 1 ? 'success' : 'info'" size="small">{{ row.status === 1 ? '在线' : '离线' }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="操作" width="200" fixed="right">
              <template #default="{ row }">
                <el-button link type="primary" @click="onShowChannels(row)">通道</el-button>
                <el-button link type="primary" @click="onEdit(row)">编辑</el-button>
                <el-button link type="danger" @click="onDelete(row)">删除</el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-tab-pane>

      <el-tab-pane label="圆形区域" name="circle">
        <el-card>
          <el-form :inline="true">
            <el-form-item label="终端手机号">
              <el-input v-model="circlePhone" placeholder="筛选 phone" clearable />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" @click="loadCircles">查询</el-button>
              <el-button @click="onAddCircle">新增</el-button>
            </el-form-item>
          </el-form>
          <el-table :data="circles" stripe border>
            <el-table-column prop="id" label="ID" width="60" />
            <el-table-column prop="phoneNumber" label="手机号" min-width="140" />
            <el-table-column prop="label" label="标签" min-width="160" />
            <el-table-column prop="centerLat" label="中心纬度" width="120" />
            <el-table-column prop="centerLon" label="中心经度" width="120" />
            <el-table-column prop="radiusM" label="半径(米)" width="100" />
            <el-table-column label="操作" width="140" fixed="right">
              <template #default="{ row }">
                <el-button link type="danger" @click="onDeleteCircle(row)">删除</el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-tab-pane>

      <el-tab-pane label="多边形区域" name="polygon">
        <el-card>
          <el-form :inline="true">
            <el-form-item label="终端手机号">
              <el-input v-model="polygonPhone" placeholder="筛选 phone" clearable />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" @click="loadPolygons">查询</el-button>
              <el-button @click="onAddPolygon">新增</el-button>
            </el-form-item>
          </el-form>
          <el-table :data="polygons" stripe border>
            <el-table-column prop="id" label="ID" width="60" />
            <el-table-column prop="phoneNumber" label="手机号" min-width="140" />
            <el-table-column prop="label" label="标签" min-width="160" />
            <el-table-column prop="pointsJson" label="点位 JSON" min-width="280" show-overflow-tooltip />
            <el-table-column label="操作" width="140" fixed="right">
              <template #default="{ row }">
                <el-button link type="danger" @click="onDeletePolygon(row)">删除</el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-tab-pane>

      <el-tab-pane label="路线" name="route">
        <el-card>
          <el-form :inline="true">
            <el-form-item label="终端手机号">
              <el-input v-model="routePhone" placeholder="筛选 phone" clearable />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" @click="loadRoutes">查询</el-button>
              <el-button @click="onAddRoute">新增</el-button>
            </el-form-item>
          </el-form>
          <el-table :data="routes" stripe border>
            <el-table-column prop="id" label="ID" width="60" />
            <el-table-column prop="phoneNumber" label="手机号" min-width="140" />
            <el-table-column prop="label" label="标签" min-width="160" />
            <el-table-column prop="waypointsJson" label="途经点 JSON" min-width="280" show-overflow-tooltip />
            <el-table-column label="操作" width="140" fixed="right">
              <template #default="{ row }">
                <el-button link type="danger" @click="onDeleteRoute(row)">删除</el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-tab-pane>

      <el-tab-pane label="终端通道" name="channel">
        <el-card>
          <el-form :inline="true">
            <el-form-item label="终端手机号">
              <el-input v-model="channelPhone" placeholder="筛选 phone" clearable />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" @click="loadChannelsFor(channelPhone)">查询</el-button>
            </el-form-item>
          </el-form>
          <el-table :data="channels" v-loading="channelLoading" stripe border>
            <el-table-column prop="id" label="ID" width="60" />
            <el-table-column prop="phoneNumber" label="手机号" min-width="140" />
            <el-table-column prop="channelId" label="通道号" width="80" />
            <el-table-column prop="channelName" label="通道名" min-width="160" />
            <el-table-column label="状态" width="100">
              <template #default="{ row }">
                <el-tag :type="row.status ? 'success' : 'info'" size="small">
                  {{ row.status ? '在线' : '离线' }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="hasAudio" label="音频" width="80" align="center">
              <template #default="{ row }">
                <el-tag v-if="row.hasAudio" type="success" size="small">是</el-tag>
                <el-tag v-else type="info" size="small">否</el-tag>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-tab-pane>
    </el-tabs>

    <jt-terminal-edit-dialog v-model="terminalEditVisible" :terminal="currentTerminal" @saved="loadData" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  getJtTerminalList,
  deleteJtTerminal,
  type JtTerminal,
  getJtAreaCircleList,
  addJtAreaCircle,
  deleteJtAreaCircle,
  getJtAreaPolygonList,
  setJtAreaPolygon,
  deleteJtAreaPolygon,
  getJtRouteList,
  setJtRoute,
  deleteJtRoute,
  getJtChannelList
} from '@/api/jtDevice'
import JtTerminalEditDialog from './TerminalEditDialog.vue'

const activeTab = ref<'terminal' | 'circle' | 'polygon' | 'route' | 'channel'>('terminal')
const loading = ref(false)
const terminals = ref<any[]>([])
const circles = ref<any[]>([])
const polygons = ref<any[]>([])
const routes = ref<any[]>([])
const circlePhone = ref('')
const polygonPhone = ref('')
const routePhone = ref('')
const channelPhone = ref('')
const terminalEditVisible = ref(false)
const currentTerminal = ref<Partial<JtTerminal>>({})

async function loadData() {
  loading.value = true
  try {
    const res = await getJtTerminalList({ page: 1, count: 200 })
    terminals.value = res.data?.list ?? []
  } finally {
    loading.value = false
  }
}

async function loadCircles() {
  if (!circlePhone.value) {
    ElMessage.warning('请填写手机号')
    return
  }
  const res = await getJtAreaCircleList(circlePhone.value)
  circles.value = res.data?.items ?? []
}

async function loadPolygons() {
  if (!polygonPhone.value) {
    ElMessage.warning('请填写手机号')
    return
  }
  const res = await getJtAreaPolygonList(polygonPhone.value)
  polygons.value = res.data?.items ?? []
}

async function loadRoutes() {
  if (!routePhone.value) {
    ElMessage.warning('请填写手机号')
    return
  }
  const res = await getJtRouteList(routePhone.value)
  routes.value = res.data?.items ?? []
}

function onAdd() {
  currentTerminal.value = {}
  terminalEditVisible.value = true
}

function onEdit(row: any) {
  currentTerminal.value = { ...row }
  terminalEditVisible.value = true
}

async function onShowChannels(row: any) {
  if (!row.id) {
    ElMessage.warning('该终端缺少主键 id')
    return
  }
  channelPhone.value = row.phoneNumber ?? ''
  activeTab.value = 'channel'
  await loadChannelsFor(row.id)
}

const channels = ref<Array<{
  id?: number
  terminalDbId?: number
  phoneNumber?: string
  channelId?: number
  channelName?: string
  hasAudio?: boolean
  status?: boolean
}>>([])
const channelLoading = ref(false)

async function loadChannelsFor(terminalDbId: number | string) {
  channelLoading.value = true
  try {
    const res = await getJtChannelList(terminalDbId)
    channels.value = ((res.data as any)?.list ?? []) as typeof channels.value
    ElMessage.success(`已加载 ${channels.value.length} 个通道`)
  } catch (e: any) {
    ElMessage.error(e?.message ?? '加载通道失败')
    channels.value = []
  } finally {
    channelLoading.value = false
  }
}

async function onDelete(row: any) {
  await ElMessageBox.confirm(`确认删除终端 ${row.phoneNumber} ？`, '确认', { type: 'warning' })
  await deleteJtTerminal(row.id ?? 0)
  ElMessage.success('已删除')
  loadData()
}

async function onAddCircle() {
  const { value } = await ElMessageBox.prompt('手机号', '新增圆形区域', {
    inputValidator: (v) => (v ? true : '请输入手机号')
  })
  await addJtAreaCircle({ phoneNumber: value, centerLat: 0, centerLon: 0, radiusM: 100, label: '未命名' })
  ElMessage.success('已新增')
  if (circlePhone.value === value) loadCircles()
}

async function onDeleteCircle(row: any) {
  await deleteJtAreaCircle(row.id ?? 0)
  ElMessage.success('已删除')
  loadCircles()
}

async function onAddPolygon() {
  const { value } = await ElMessageBox.prompt('手机号', '新增多边形', {
    inputValidator: (v) => (v ? true : '请输入手机号')
  })
  await setJtAreaPolygon({ phoneNumber: value, pointsJson: '[]', label: '未命名' })
  ElMessage.success('已新增')
  if (polygonPhone.value === value) loadPolygons()
}

async function onDeletePolygon(row: any) {
  await ElMessageBox.confirm('确认删除？', '确认', { type: 'warning' })
  await deleteJtAreaPolygon(row.id ?? 0)
  ElMessage.success('已删除')
  loadPolygons()
}

async function onAddRoute() {
  const { value } = await ElMessageBox.prompt('手机号', '新增路线', {
    inputValidator: (v) => (v ? true : '请输入手机号')
  })
  await setJtRoute({ phoneNumber: value, waypointsJson: '[]', label: '未命名' })
  ElMessage.success('已新增')
  if (routePhone.value === value) loadRoutes()
}

async function onDeleteRoute(row: any) {
  await ElMessageBox.confirm('确认删除？', '确认', { type: 'warning' })
  await deleteJtRoute(row.id ?? 0)
  ElMessage.success('已删除')
  loadRoutes()
}

onMounted(loadData)
</script>

<style scoped>
.jt-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
