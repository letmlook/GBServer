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
        <el-table-column label="操作" width="280" fixed="right">
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
    <el-dialog v-model="pwdVisible" title="修改密码" width="420px" @open="onPwdOpen">
      <el-form ref="pwdFormRef" :model="pwdForm" :rules="pwdRules" label-width="100px">
        <el-form-item label="用户名">
          <span class="mono">{{ pwdTarget?.username ?? '-' }}</span>
        </el-form-item>
        <el-form-item label="原密码" prop="oldPassword">
          <el-input v-model="pwdForm.oldPassword" type="password" show-password />
        </el-form-item>
        <el-form-item label="新密码" prop="password">
          <el-input v-model="pwdForm.password" type="password" show-password />
        </el-form-item>
        <el-form-item label="确认新密码" prop="password2">
          <el-input v-model="pwdForm.password2" type="password" show-password />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="pwdVisible = false">取消</el-button>
        <el-button type="primary" :loading="pwdSaving" @click="onPwdSave">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox, type FormInstance, type FormRules } from 'element-plus'
import { getUserList, deleteUser, changePassword, changePasswordForAdmin, changePushKey, getRoleAll } from '@/api/user'
import UserAddDialog from './AddDialog.vue'

const loading = ref(false)
const rows = ref<any[]>([])
const roles = ref<{ id: number; name: string }[]>([])
const addVisible = ref(false)
const myUserId = ref<number>()

const pwdVisible = ref(false)
const pwdTarget = ref<any>(null)
const pwdSaving = ref(false)
const pwdFormRef = ref<FormInstance>()
const pwdForm = reactive({ oldPassword: '', password: '', password2: '' })
const pwdRules: FormRules = {
  oldPassword: [{ required: true, message: '请输入原密码', trigger: 'blur' }],
  password: [
    { required: true, message: '请输入新密码', trigger: 'blur' },
    { min: 6, message: '密码至少 6 位', trigger: 'blur' }
  ],
  password2: [
    { required: true, message: '请再次输入新密码', trigger: 'blur' },
    {
      validator: (_rule, value, callback) => {
        if (value !== pwdForm.password) callback(new Error('两次输入不一致'))
        else callback()
      },
      trigger: 'blur'
    }
  ]
}

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

function onPwdOpen() {
  pwdForm.oldPassword = ''
  pwdForm.password = ''
  pwdForm.password2 = ''
}

async function onPwdSave() {
  if (!pwdFormRef.value) return
  await pwdFormRef.value.validate()
  pwdSaving.value = true
  try {
    await changePassword({
      oldPassword: pwdForm.oldPassword,
      password: pwdForm.password
    })
    ElMessage.success('密码已修改，请重新登录')
    pwdVisible.value = false
  } catch (e: any) {
    ElMessage.error(e?.message ?? '密码修改失败')
  } finally {
    pwdSaving.value = false
  }
}

function onChangePwd(row: any) {
  pwdTarget.value = row
  pwdVisible.value = true
}

async function onResetPwd(row: any) {
  const { value } = await ElMessageBox.prompt('新密码（至少 6 位）', `重置 ${row.username} 的密码`, {
    inputValidator: (v) => (v && v.length >= 6 ? true : '密码至少 6 位')
  })
  await changePasswordForAdmin({ userId: row.id, password: value })
  ElMessage.success('密码已重置')
}

async function onRegenKey(row: any) {
  await ElMessageBox.confirm(`确认重置用户 ${row.username} 的 PushKey？`, '确认', { type: 'warning' })
  // 生成 32 字节随机 hex key（与服务端 pushKey 长度对齐）
  const buf = new Uint8Array(16)
  crypto.getRandomValues(buf)
  const newKey = Array.from(buf).map((b) => b.toString(16).padStart(2, '0')).join('')
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
