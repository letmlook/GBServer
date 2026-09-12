<template>
  <div class="region-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">行政区划 / 业务分组</h1>
        <p class="page-subtitle">GB/T 28181 目录树 · 通道归类</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button v-if="tab === 'region'" @click="onSync">同步行政区划</el-button>
        <el-button type="primary" :icon="Plus" @click="onAdd(null)">新增</el-button>
      </div>
    </div>

    <el-card>
      <el-tabs v-model="tab" @tab-change="onTabChange">
        <el-tab-pane label="行政区划" name="region">
          <el-input
            v-model="keyword"
            placeholder="按名称 / 国标编码搜索"
            clearable
            style="width: 280px; margin-bottom: 8px"
            @keyup.enter="loadData"
            @clear="loadData"
          />
          <el-tree
            :data="regionTree"
            node-key="id"
            default-expand-all
            :props="{ label: 'name', children: 'children' }"
            :filter-node-method="filterNode"
            ref="regionTreeRef"
            v-loading="loading"
          >
            <template #default="{ data }">
              <span class="node-row">
                <span class="node-name">{{ data.name }}</span>
                <span class="mono node-id">{{ data.deviceId }}</span>
                <span class="node-ops">
                  <el-button link type="primary" size="small" @click.stop="onAdd(data)">新增子节点</el-button>
                  <el-button link type="primary" size="small" @click.stop="onEdit(data)">编辑</el-button>
                  <el-button link type="danger" size="small" @click.stop="onDelete(data)">删除</el-button>
                </span>
              </span>
            </template>
          </el-tree>
          <el-empty v-if="!loading && !regionTree.length" description="暂无行政区划，可点「同步行政区划」导入" />
        </el-tab-pane>

        <el-tab-pane label="业务分组" name="group">
          <el-input
            v-model="keyword"
            placeholder="按名称 / 国标编码搜索"
            clearable
            style="width: 280px; margin-bottom: 8px"
            @keyup.enter="loadData"
            @clear="loadData"
          />
          <el-tree
            :data="groupTree"
            node-key="id"
            default-expand-all
            :props="{ label: 'name', children: 'children' }"
            :filter-node-method="filterNode"
            ref="groupTreeRef"
            v-loading="loading"
          >
            <template #default="{ data }">
              <span class="node-row">
                <span class="node-name">{{ data.name }}</span>
                <span class="mono node-id">{{ data.deviceId }}</span>
                <el-tag v-if="data.businessGroup" size="small" type="info">{{ data.businessGroup }}</el-tag>
                <span class="node-ops">
                  <el-button link type="primary" size="small" @click.stop="onAdd(data)">新增子节点</el-button>
                  <el-button link type="primary" size="small" @click.stop="onEdit(data)">编辑</el-button>
                  <el-button link type="danger" size="small" @click.stop="onDelete(data)">删除</el-button>
                </span>
              </span>
            </template>
          </el-tree>
          <el-empty v-if="!loading && !groupTree.length" description="暂无业务分组" />
        </el-tab-pane>
      </el-tabs>
    </el-card>

    <node-edit-dialog
      v-model="editVisible"
      :kind="tab"
      :node="currentNode"
      :default-parent-id="defaultParentId"
      :tree="tab === 'region' ? regionTree : groupTree"
      @saved="loadData"
    />
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox, type TreeInstance } from 'element-plus'
import {
  getRegionTreeList,
  getGroupTreeList,
  deleteRegion,
  deleteGroup,
  syncRegion
} from '@/api/region'
import NodeEditDialog from './NodeEditDialog.vue'

const tab = ref<'region' | 'group'>('region')
const loading = ref(false)
// 两个 tab **必须各自持有数据**：el-tabs 默认把两个 pane 都渲染在 DOM 里
// （非活动的只是 display:none），若共用一个 `tree`，切到业务分组时被隐藏的
// 行政区划树也会跟着渲染分组数据 —— 一个标题下的两棵树状态互相污染。
const regionTree = ref<any[]>([])
const groupTree = ref<any[]>([])
const keyword = ref('')
// 两个 tab 各有一棵 el-tree。**不能共用一个 ref 名**：Vue 只会保留最后注册的那一个，
// 于是 filter() 作用在隐藏的另一棵树上，当前 tab 的搜索框看起来完全没反应。
const regionTreeRef = ref<TreeInstance>()
const groupTreeRef = ref<TreeInstance>()
const editVisible = ref(false)
const currentNode = ref<Record<string, any> | null>(null)
const defaultParentId = ref<number | undefined>(undefined)

async function loadData() {
  loading.value = true
  try {
    // 这里用 tree/list（一次拿全树）而不是 tree/query：
    // 树控件本来就要完整层级，分页的 tree/query 更适合"按父节点懒加载"的场景。
    const res = tab.value === 'region' ? await getRegionTreeList() : await getGroupTreeList()
    if (tab.value === 'region') regionTree.value = res.data ?? []
    else groupTree.value = res.data ?? []
  } finally {
    loading.value = false
  }
}

function onTabChange() {
  keyword.value = ''
  loadData()
}

function filterNode(value: string, data: Record<string, any>) {
  if (!value) return true
  const kw = value.trim().toLowerCase()
  return (
    String(data.name ?? '').toLowerCase().includes(kw) ||
    String(data.deviceId ?? '').toLowerCase().includes(kw)
  )
}

watch(keyword, (v) => {
  const tree = tab.value === 'region' ? regionTreeRef.value : groupTreeRef.value
  tree?.filter(v)
})

// 切 tab 后把关键字重新应用到当前这棵树
watch(tab, () => nextTick(() => {
  const tree = tab.value === 'region' ? regionTreeRef.value : groupTreeRef.value
  tree?.filter(keyword.value)
}))

function onAdd(parent: Record<string, any> | null) {
  currentNode.value = null
  defaultParentId.value = parent?.id
  editVisible.value = true
}

function onEdit(node: Record<string, any>) {
  currentNode.value = { ...node }
  defaultParentId.value = undefined
  editVisible.value = true
}

async function onDelete(node: Record<string, any>) {
  const hasChildren = (node.children?.length ?? 0) > 0
  await ElMessageBox.confirm(
    hasChildren
      ? `「${node.name}」下还有 ${node.children.length} 个子节点，删除后子节点会变成顶级节点，确认删除？`
      : `确认删除「${node.name}」？`,
    '确认',
    { type: 'warning' }
  )
  // 此前前端用 GET 打只注册了 DELETE 的路由 → 405，删除永远失败
  if (tab.value === 'region') await deleteRegion(node.id)
  else await deleteGroup(node.id)
  ElMessage.success('已删除')
  loadData()
}

async function onSync() {
  const res = await syncRegion()
  const count = (res.data as any)?.count ?? 0
  ElMessage.success(`同步完成，新增 ${count} 个区域`)
  await loadData()
  await nextTick()
}

onMounted(loadData)
</script>

<style scoped>
.region-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.node-row { display: flex; align-items: center; gap: 10px; width: 100%; }
.node-name { min-width: 140px; }
.node-id { color: var(--el-text-color-secondary); }
.node-ops { margin-left: auto; opacity: 0.35; transition: opacity 0.15s; }
.node-row:hover .node-ops { opacity: 1; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
