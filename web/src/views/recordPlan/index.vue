<template>
  <div class="record-plan-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">录像计划</h1>
        <p class="page-subtitle">按时段自动拉起设备流并录像 · 通道关联</p>
      </div>
      <div class="page-actions">
        <el-button :icon="Refresh" @click="loadData">刷新</el-button>
        <el-button type="primary" :icon="Plus" @click="onAdd">新增计划</el-button>
      </div>
    </div>

    <el-card>
      <el-form :inline="true" size="small" class="search-bar">
        <el-form-item label="关键字">
          <el-input
            v-model="query"
            placeholder="计划名称"
            clearable
            style="width: 200px"
            @keyup.enter="onSearch"
          />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="onSearch">查询</el-button>
        </el-form-item>
      </el-form>

      <el-table :data="rows" v-loading="loading" stripe border>
        <el-table-column prop="id" label="ID" width="70" />
        <el-table-column prop="name" label="计划名称" min-width="160" />
        <el-table-column label="录像时段" min-width="320">
          <template #default="{ row }">
            <span class="windows">{{ summarizePlanItems(row.planItemList) }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="channelCount" label="关联通道" width="100" align="center" />
        <el-table-column prop="updateTime" label="更新时间" width="180" />
        <el-table-column prop="createTime" label="创建时间" width="180" />
        <el-table-column label="操作" width="220" fixed="right">
          <template #default="{ row }">
            <el-button link type="primary" @click="onLink(row)">关联通道</el-button>
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
        :page-sizes="[15, 30, 50, 100]"
        @current-change="onPageChange"
        @size-change="onSizeChange"
      />
    </el-card>

    <record-plan-edit-dialog v-model="editVisible" :plan="currentRow" @saved="loadData" />
    <link-channel-dialog
      v-model="linkVisible"
      :plan-id="currentPlan?.id"
      :plan-name="currentPlan?.name ?? ''"
      @changed="loadData"
    />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Plus, Refresh } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  deleteRecordPlan,
  getRecordPlanList,
  summarizePlanItems,
  type RecordPlan
} from '@/api/recordPlan'
import RecordPlanEditDialog from './EditDialog.vue'
import LinkChannelDialog from './LinkChannelDialog.vue'

const rows = ref<RecordPlan[]>([])
const loading = ref(false)
const query = ref('')
const page = ref(1)
const count = ref(15)
const total = ref(0)

const editVisible = ref(false)
const linkVisible = ref(false)
const currentRow = ref<Partial<RecordPlan> | null>(null)
const currentPlan = ref<RecordPlan | null>(null)

async function loadData() {
  loading.value = true
  try {
    const res = await getRecordPlanList({
      page: page.value,
      count: count.value,
      query: query.value || undefined
    })
    rows.value = res.data?.list ?? []
    total.value = res.data?.total ?? 0
  } catch (e: any) {
    ElMessage.error(e?.message ?? '加载录像计划失败')
    rows.value = []
    total.value = 0
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

function onSizeChange(c: number) {
  count.value = c
  page.value = 1
  loadData()
}

function onAdd() {
  currentRow.value = null
  editVisible.value = true
}

function onEdit(row: RecordPlan) {
  currentRow.value = { ...row }
  editVisible.value = true
}

function onLink(row: RecordPlan) {
  currentPlan.value = row
  linkVisible.value = true
}

async function onDelete(row: RecordPlan) {
  await ElMessageBox.confirm(`确认删除计划「${row.name}」？关联的通道会一并解除。`, '确认', {
    type: 'warning'
  })
  try {
    await deleteRecordPlan(row.id ?? 0)
    ElMessage.success('已删除')
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '删除失败')
  }
}

onMounted(loadData)
</script>

<style scoped>
.record-plan-page {
  padding: 16px;
}
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  margin-bottom: 12px;
}
.page-title {
  font-size: 20px;
  font-weight: 600;
  margin: 0;
}
.page-subtitle {
  color: var(--el-text-color-secondary);
  font-size: 12px;
  margin-top: 4px;
}
.search-bar {
  margin-bottom: 8px;
}
.windows {
  font-size: 12px;
  color: var(--el-text-color-regular);
  line-height: 1.5;
}
.pager {
  margin-top: 12px;
  justify-content: flex-end;
}
</style>
