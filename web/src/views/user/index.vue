<template>
  <div class="user-page">
    <div class="page-header">
      <div class="page-actions">
        <el-input
          v-model="keyword"
          class="search"
          placeholder="搜索用户名"
          clearable
          :prefix-icon="Search"
          @keyup.enter="onSearch"
          @clear="onSearch"
        />
        <el-button @click="onSearch">搜索</el-button>
        <el-button @click="loadData">刷新</el-button>
        <el-button v-if="isAdmin" type="primary" :icon="Plus" @click="onAdd">新增用户</el-button>
      </div>
    </div>

    <el-card v-if="!authChecked">
      <el-skeleton :rows="4" animated />
    </el-card>

    <!--
      非管理员：后端对列表/角色接口均返回 403（用户列表含每人 pushKey，属敏感数据）。
      此时不再展示空表格，而是给出明确说明 —— 否则页面看起来像"加载失败"。
    -->
    <el-card v-else-if="!isAdmin">
      <el-empty description="需要管理员权限才能查看用户列表">
        <el-button @click="router.push('/dashboard')">返回控制台</el-button>
      </el-empty>
    </el-card>

    <el-card v-else>
      <el-table :data="rows" v-loading="loading" stripe border>
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="username" label="用户名" min-width="160" />
        <el-table-column label="角色" min-width="120">
          <template #default="{ row }">{{ row.role?.name ?? '-' }}</template>
        </el-table-column>
        <el-table-column prop="pushKey" label="PushKey" min-width="280">
          <template #default="{ row }">
            <span class="mono">{{ row.pushKey ?? '-' }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="createTime" label="创建时间" min-width="180">
          <template #default="{ row }"><span class="mono">{{ row.createTime ?? '-' }}</span></template>
        </el-table-column>
        <!--
          操作列：
          * 自己那一行 —— 改密（后端 changePassword 只认 token，改的必然是本人）+ 编辑。
          * 其他行（仅管理员）—— 编辑 / 重置密码 / 重置 PushKey / 删除。
        -->
        <el-table-column label="操作" width="340" fixed="right">
          <template #default="{ row }">
            <div class="row-actions">
              <template v-if="isSelf(row)">
                <el-button link type="primary" @click="onChangeMyPwd(row)">改密</el-button>
                <el-button link type="primary" @click="onEdit(row)">编辑</el-button>
              </template>
              <template v-else>
                <el-button link type="primary" @click="onEdit(row)">编辑</el-button>
                <el-button link type="primary" @click="onResetPwd(row)">重置密码</el-button>
                <el-button link type="warning" @click="onRegenKey(row)">重置 PushKey</el-button>
                <el-button link type="danger" @click="onDelete(row)">删除</el-button>
              </template>
            </div>
          </template>
        </el-table-column>
      </el-table>

      <el-pagination
        class="pager"
        layout="total, sizes, prev, pager, next"
        :total="total"
        :current-page="page"
        :page-size="count"
        :page-sizes="[10, 20, 50, 100]"
        @current-change="onPageChange"
        @size-change="onSizeChange"
      />
    </el-card>

    <user-add-dialog v-model="addVisible" :roles="roles" @saved="loadData" />
    <user-edit-dialog v-model="editVisible" :row="editTarget" :roles="roles" @saved="loadData" />

    <!--
      自助改密：后端 `POST /api/user/changePassword` **从 token 认人**、不接受
      userId，所以这里只能改当前登录账号的密码。弹窗标题与提示都写明这一点，
      避免管理员误以为在改别人（此前按钮对所有行都显示、标题还显示所选行的人）。
    -->
    <el-dialog v-model="pwdVisible" title="修改我的密码" width="420px" @open="onPwdOpen">
      <el-alert
        type="info"
        :closable="false"
        show-icon
        title="此操作修改的是当前登录账号的密码，不是表格里其他人的。"
        class="pwd-tip"
      />
      <el-form ref="pwdFormRef" :model="pwdForm" :rules="pwdRules" label-width="100px">
        <el-form-item label="账号">
          <span class="mono">{{ myName || '-' }}</span>
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
import { useRouter } from 'vue-router'
import { Plus, Search } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox, type FormInstance, type FormRules } from 'element-plus'
import {
  getUserList,
  getUserInfo,
  deleteUser,
  changePassword,
  changePasswordForAdmin,
  changePushKey,
  getRoleAll
} from '@/api/user'
import UserAddDialog from './AddDialog.vue'
import UserEditDialog from './EditDialog.vue'

const loading = ref(false)
/** 身份确认前不渲染表格，避免非管理员先看到一次 403 报错。 */
const authChecked = ref(false)
const rows = ref<any[]>([])
const roles = ref<{ id: number; name: string }[]>([])
const addVisible = ref(false)
const editVisible = ref(false)
const editTarget = ref<any>(null)
const router = useRouter()

/** 当前登录账号。后端 changePassword 只认 token，所以「改密」只作用于它。 */
const myUserId = ref<number>()
const myName = ref('')
const isAdmin = ref(false)

const keyword = ref('')

const pwdVisible = ref(false)
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

// 后端 `GET /api/user/users` 会把 count 截断到 100，所以这里必须真分页，
// 否则第 101 个及之后的用户永远不可见。
const page = ref(1)
const count = ref(20)
const total = ref(0)

/** 与后端 `authz::is_admin_role` 同口径：authority == '0' 视为管理员。 */
function isSelf(row: any) {
  return myUserId.value != null && row?.id === myUserId.value
}

async function loadData() {
  loading.value = true
  try {
    const res = await getUserList({
      page: page.value,
      count: count.value,
      query: keyword.value.trim() || undefined
    })
    rows.value = res.data?.list ?? []
    total.value = res.data?.total ?? 0
  } catch (e: any) {
    ElMessage.error(e?.message ?? '加载用户失败')
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
  addVisible.value = true
}

function onEdit(row: any) {
  editTarget.value = row
  editVisible.value = true
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

function onChangeMyPwd(_row: any) {
  // 忽略 row：接口只改当前登录账号，靶子由 token 决定。
  pwdVisible.value = true
}

async function onResetPwd(row: any) {
  const { value } = await ElMessageBox.prompt('新密码（至少 6 位）', `重置 ${row.username} 的密码`, {
    inputValidator: (v) => (v && v.length >= 6 ? true : '密码至少 6 位')
  })
  try {
    await changePasswordForAdmin({ userId: row.id, password: value })
    ElMessage.success('密码已重置')
  } catch (e: any) {
    ElMessage.error(e?.message ?? '重置失败')
  }
}

async function onRegenKey(row: any) {
  await ElMessageBox.confirm(`确认重置用户 ${row.username} 的 PushKey？`, '确认', { type: 'warning' })
  // 生成 16 字节随机 hex key（与服务端 pushKey 长度对齐）
  const buf = new Uint8Array(16)
  crypto.getRandomValues(buf)
  const newKey = Array.from(buf).map((b) => b.toString(16).padStart(2, '0')).join('')
  try {
    await changePushKey({ userId: row.id, pushKey: newKey })
    ElMessage.success(`新 PushKey: ${newKey}`)
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '重置失败')
  }
}

async function onDelete(row: any) {
  await ElMessageBox.confirm(`确认删除用户 ${row.username} ？`, '确认', { type: 'warning' })
  try {
    await deleteUser(row.id ?? 0)
    ElMessage.success('已删除')
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '删除失败')
  }
}

onMounted(async () => {
  // 先认人再拉列表：决定整页是否可访问、以及操作列展示哪些按钮。
  try {
    const me = await getUserInfo()
    const data: any = me?.data
    myUserId.value = data?.id
    myName.value = data?.username ?? ''
    isAdmin.value = data?.role?.authority === '0' || data?.role?.id === 1
  } catch {
    isAdmin.value = false
  }
  authChecked.value = true

  if (!isAdmin.value) return

  await loadData()

  getRoleAll()
    .then((r) => (roles.value = (r.data as any[]) ?? []))
    .catch(() => {})
})
</script>

<style scoped>
.pager {
  margin-top: 12px;
  justify-content: flex-end;
}
.user-page { padding: 16px; }
.page-header { display: flex; justify-content: flex-end; align-items: flex-end; margin-bottom: 12px; gap: 12px; }
.page-actions { display: flex; align-items: center; gap: 8px; }
.search { width: 200px; }
/* 操作列按单行排布：默认 el-button 之间的 margin 会让 4 个按钮换行错位。 */
.row-actions { display: flex; align-items: center; flex-wrap: nowrap; white-space: nowrap; }
.row-actions :deep(.el-button + .el-button) { margin-left: 8px; }
.pwd-tip { margin-bottom: 12px; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: var(--text-sm); }
.muted { color: var(--el-text-color-placeholder); }
</style>
