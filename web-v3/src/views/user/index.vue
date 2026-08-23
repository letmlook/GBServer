<template>
  <div class="user-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">用户管理</h1>
        <p class="page-subtitle">账号 · 角色 · 密码 · PushKey</p>
      </div>
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button type="primary" :icon="Plus" @click="onAdd">新增用户</el-button>
      </div>
    </div>

    <el-card>
      <el-table :data="rows" v-loading="loading" stripe border>
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="username" label="用户名" min-width="160" />
        <el-table-column prop="roleName" label="角色" min-width="120" />
        <el-table-column prop="pushKey" label="PushKey" min-width="280">
          <template #default="{ row }">
            <span class="mono">{{ row.pushKey ?? '-' }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="createTime" label="创建时间" min-width="180">
          <template #default="{ row }"><span class="mono">{{ row.createTime ?? '-' }}</span></template>
        </el-table-column>
        <el-table-column label="操作" width="240" fixed="right">
          <template #default="{ row }">
            <el-button link type="primary" @click="onChangePwd(row)">改密</el-button>
            <el-button link type="primary" @click="onResetPwd(row)">重置</el-button>
            <el-button link type="warning" @click="onRegenKey(row)">重置 PushKey</el-button>
            <el-button link type="danger" @click="onDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <user-add-dialog v-model="addVisible" :roles="roles" @saved="loadData" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { getUserList, deleteUser, changePasswordForAdmin, changePushKey, getRoleAll } from '@/api/user'
import UserAddDialog from './AddDialog.vue'

const loading = ref(false)
const rows = ref<any[]>([])
const roles = ref<{ id: number; name: string }[]>([])
const addVisible = ref(false)
const myUserId = ref<number>()

async function loadData() {
  loading.value = true
  try {
    const res = await getUserList({ page: 1, count: 200 })
    rows.value = res.data?.list ?? []
  } finally {
    loading.value = false
  }
}

function onAdd() {
  addVisible.value = true
}

async function onChangePwd(_row: any) {
  ElMessage.info('请通过修改密码对话框修改（前端占位）')
}

async function onResetPwd(row: any) {
  const { value } = await ElMessageBox.prompt('新密码', `重置 ${row.username} 的密码`, {
    inputValidator: (v) => (v ? true : '请输入新密码')
  })
  await changePasswordForAdmin({ userId: row.id, password: value })
  ElMessage.success('密码已重置')
}

async function onRegenKey(row: any) {
  await ElMessageBox.confirm(`确认重置用户 ${row.username} 的 PushKey？`, '确认', { type: 'warning' })
  const newKey = Math.random().toString(36).slice(2) + Math.random().toString(36).slice(2)
  await changePushKey({ userId: row.id, pushKey: newKey })
  ElMessage.success(`新 PushKey: ${newKey}`)
  loadData()
}

async function onDelete(row: any) {
  await ElMessageBox.confirm(`确认删除用户 ${row.username} ？`, '确认', { type: 'warning' })
  await deleteUser(row.id ?? 0)
  ElMessage.success('已删除')
  loadData()
}

onMounted(async () => {
  await loadData()
  getRoleAll().then((r) => (roles.value = (r.data as any[]) ?? [])).catch(() => {})
})
</script>

<style scoped>
.user-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.page-subtitle { color: var(--el-text-color-secondary); font-size: 12px; margin-top: 4px; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
