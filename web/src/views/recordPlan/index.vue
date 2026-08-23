<template>
  <div class="record-plan-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">录像计划</h1>
        <p class="page-subtitle">定时录像 · 通道关联</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button type="primary" :icon="Plus" @click="onAdd">新增计划</el-button>
      </div>
    </div>

    <el-card>
      <el-table :data="rows" v-loading="loading" stripe border>
        <el-table-column type="index" label="#" width="50" />
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="name" label="计划名称" min-width="200" />
        <el-table-column prop="planType" label="类型" width="100" />
        <el-table-column prop="startTime" label="开始" width="100" />
        <el-table-column prop="endTime" label="结束" width="100" />
        <el-table-column label="周日" width="60" align="center">
          <template #default="{ row }">{{ row.sun ? '✓' : '·' }}</template>
        </el-table-column>
        <el-table-column label="周一" width="60" align="center">
          <template #default="{ row }">{{ row.mon ? '✓' : '·' }}</template>
        </el-table-column>
        <el-table-column label="周二" width="60" align="center">
          <template #default="{ row }">{{ row.tue ? '✓' : '·' }}</template>
        </el-table-column>
        <el-table-column label="周三" width="60" align="center">
          <template #default="{ row }">{{ row.wed ? '✓' : '·' }}</template>
        </el-table-column>
        <el-table-column label="周四" width="60" align="center">
          <template #default="{ row }">{{ row.thu ? '✓' : '·' }}</template>
        </el-table-column>
        <el-table-column label="周五" width="60" align="center">
          <template #default="{ row }">{{ row.fri ? '✓' : '·' }}</template>
        </el-table-column>
        <el-table-column label="周六" width="60" align="center">
          <template #default="{ row }">{{ row.sat ? '✓' : '·' }}</template>
        </el-table-column>
        <el-table-column label="启用" width="80">
          <template #default="{ row }">
            <el-switch v-model="row.enable" @change="onToggle(row)" />
          </template>
        </el-table-column>
        <el-table-column label="操作" width="220" fixed="right">
          <template #default="{ row }">
            <el-button link type="primary" @click="onEdit(row)">编辑</el-button>
            <el-button link type="primary" @click="onLink(row)">关联通道</el-button>
            <el-button link type="danger" @click="onDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <record-plan-edit-dialog v-model="editVisible" :plan="currentRow" @saved="loadData" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { getRecordPlanList, deleteRecordPlan, addRecordPlan, updateRecordPlan, type RecordPlan } from '@/api/recordPlan'
import RecordPlanEditDialog from './EditDialog.vue'

const loading = ref(false)
const rows = ref<RecordPlan[]>([])
const editVisible = ref(false)
const currentRow = ref<Partial<RecordPlan>>({})

async function loadData() {
  loading.value = true
  try {
    const res = await getRecordPlanList({ page: 1, count: 200 })
    rows.value = res.data?.list ?? []
  } finally {
    loading.value = false
  }
}

function onAdd() {
  currentRow.value = { mon: true, tue: true, wed: true, thu: true, fri: true, sat: true, sun: true, enable: true, startTime: '00:00:00', endTime: '23:59:59' }
  editVisible.value = true
}

function onEdit(row: RecordPlan) {
  currentRow.value = { ...row }
  editVisible.value = true
}

async function onToggle(row: RecordPlan) {
  try {
    await updateRecordPlan({ id: row.id, enable: row.enable })
    ElMessage.success(row.enable ? '已启用' : '已停用')
  } catch (e: any) {
    ElMessage.error(e?.message ?? '切换失败')
    row.enable = !row.enable
  }
}

function onLink(row: RecordPlan) {
  ElMessage.info(`关联通道 - 通过 /api/record/plan/link (planId=${row.id}) 实现，可调 linkChannels API`)
}

async function onDelete(row: RecordPlan) {
  await ElMessageBox.confirm(`确认删除计划 ${row.name} ？`, '确认', { type: 'warning' })
  await deleteRecordPlan(row.id ?? 0)
  ElMessage.success('已删除')
  loadData()
}

onMounted(loadData)
</script>

<style scoped>
.record-plan-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
</style>
