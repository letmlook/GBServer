<template>
  <el-dialog
    :model-value="modelValue"
    :title="`关联通道 — ${planName}`"
    width="900px"
    @update:model-value="(v: boolean) => emit('update:modelValue', v)"
    @open="onOpen"
  >
    <el-tabs v-model="hasLink" @tab-change="onSearch">
      <el-tab-pane label="未关联" name="false" />
      <el-tab-pane label="已关联" name="true" />
    </el-tabs>

    <el-form :inline="true" size="small">
      <el-form-item label="关键字">
        <el-input
          v-model="query"
          placeholder="通道名 / 编号"
          clearable
          style="width: 180px"
          @keyup.enter="onSearch"
        />
      </el-form-item>
      <el-form-item label="在线">
        <el-select v-model="online" clearable placeholder="全部" style="width: 110px" @change="onSearch">
          <el-option label="在线" value="true" />
          <el-option label="离线" value="false" />
        </el-select>
      </el-form-item>
      <el-form-item>
        <el-button type="primary" @click="onSearch">查询</el-button>
      </el-form-item>
      <el-form-item>
        <template v-if="hasLink === 'false'">
          <el-button type="primary" :loading="saving" @click="onLinkSelected">添加</el-button>
          <el-button @click="onLinkAll(true)">添加所有通道</el-button>
        </template>
        <template v-else>
          <el-button type="danger" :loading="saving" @click="onUnlinkSelected">移除</el-button>
          <el-button @click="onLinkAll(false)">移除所有通道</el-button>
        </template>
      </el-form-item>
    </el-form>

    <el-table
      :data="rows"
      v-loading="loading"
      stripe
      border
      max-height="420"
      @selection-change="(arr: RecordPlanChannel[]) => (selected = arr)"
    >
      <el-table-column type="selection" width="46" />
      <el-table-column prop="gbName" label="通道名" min-width="200" />
      <el-table-column prop="gbDeviceId" label="通道编号" min-width="200" />
      <el-table-column prop="gbManufacturer" label="厂家" min-width="120" />
      <el-table-column label="状态" width="90" align="center">
        <template #default="{ row }">
          <el-tag :type="row.gbStatus === 'ON' ? 'success' : 'info'" size="small">
            {{ row.gbStatus === 'ON' ? '在线' : '离线' }}
          </el-tag>
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

    <template #footer>
      <el-button @click="emit('update:modelValue', false)">关闭</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  linkPlan,
  queryPlanChannels,
  type RecordPlanChannel
} from '@/api/recordPlan'

const props = defineProps<{
  modelValue: boolean
  planId?: number
  planName?: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'changed'): void
}>()

const hasLink = ref<'true' | 'false'>('false')
const query = ref('')
const online = ref('')
const rows = ref<RecordPlanChannel[]>([])
const selected = ref<RecordPlanChannel[]>([])
const page = ref(1)
const count = ref(15)
const total = ref(0)
const loading = ref(false)
const saving = ref(false)

async function load() {
  if (!props.planId) return
  loading.value = true
  try {
    // 列表接口返回的 total/list 与 WVP PageInfo 同构
    const res = await queryPlanChannels({
      page: page.value,
      count: count.value,
      planId: props.planId,
      query: query.value || undefined,
      online: online.value || undefined,
      hasLink: hasLink.value
    })
    rows.value = res.data?.list ?? []
    total.value = res.data?.total ?? 0
  } catch (e: any) {
    ElMessage.error(e?.message ?? '加载通道失败')
    rows.value = []
    total.value = 0
  } finally {
    loading.value = false
  }
}

function onOpen() {
  hasLink.value = 'false'
  query.value = ''
  online.value = ''
  page.value = 1
  selected.value = []
  load()
}

function onSearch() {
  page.value = 1
  load()
}

function onPageChange(p: number) {
  page.value = p
  load()
}

function onSizeChange(c: number) {
  count.value = c
  page.value = 1
  load()
}

async function doLink(payload: { planId?: number; channelIds?: number[]; allLink?: boolean }, tip: string) {
  saving.value = true
  try {
    await linkPlan(payload)
    ElMessage.success(tip)
    emit('changed')
    await load()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '操作失败')
  } finally {
    saving.value = false
  }
}

async function onLinkSelected() {
  if (selected.value.length === 0) {
    ElMessage.warning('请选择要关联的通道')
    return
  }
  await doLink(
    { planId: props.planId, channelIds: selected.value.map((c) => c.gbId) },
    `已关联 ${selected.value.length} 个通道`
  )
}

async function onUnlinkSelected() {
  if (selected.value.length === 0) {
    ElMessage.warning('请选择要移除的通道')
    return
  }
  // 不带 planId 的 channelIds = 取消关联（WVP 语义）
  await doLink(
    { channelIds: selected.value.map((c) => c.gbId) },
    `已移除 ${selected.value.length} 个通道`
  )
}

async function onLinkAll(all: boolean) {
  const text = all
    ? '将把所有通道关联到本计划（包括已关联到其它计划的通道），确定继续？'
    : '确定移除本计划的全部通道关联？'
  await ElMessageBox.confirm(text, '确认', { type: 'warning' })
  await doLink({ planId: props.planId, allLink: all }, all ? '已关联全部通道' : '已移除全部关联')
}

watch(
  () => props.modelValue,
  (v) => {
    if (v) onOpen()
  }
)
</script>

<style scoped>
.pager {
  margin-top: 10px;
  justify-content: flex-end;
}
</style>
