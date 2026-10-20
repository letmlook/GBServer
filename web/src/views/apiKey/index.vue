<template>
  <div class="apikey-page">
    <div class="page-header">
      <div class="page-actions">
        <el-button @click="loadData">刷新</el-button>
        <el-button v-if="isAdmin" type="primary" :icon="Plus" @click="openAdd">新增 API Key</el-button>
      </div>
    </div>

    <el-card v-if="!authChecked">
      <el-skeleton :rows="4" animated />
    </el-card>

    <!--
      后端 7 条端点全部要求管理员（`authz::require_admin`）。
      非管理员看到的是 403，不是空列表 —— 所以这里明确说明，不装成"没有数据"。
    -->
    <el-card v-else-if="!isAdmin">
      <el-empty description="需要管理员权限才能管理 API Key">
        <el-button @click="router.push('/dashboard')">返回控制台</el-button>
      </el-empty>
    </el-card>

    <el-card v-else>
      <el-table table-layout="auto" :data="rows" v-loading="loading" stripe border>
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="app" label="应用" min-width="140">
          <template #default="{ row }">{{ row.app || '-' }}</template>
        </el-table-column>
        <el-table-column prop="userId" label="归属用户" width="100">
          <template #default="{ row }">{{ row.userId ?? '-' }}</template>
        </el-table-column>
        <el-table-column label="密钥" min-width="180">
          <template #default="{ row }">
            <!-- 列表里后端只给掩码；真实密钥只在新增/重置时给过一次。 -->
            <span class="mono">{{ row.apiKey ?? '-' }}</span>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="110" align="center">
          <template #default="{ row }">
            <el-tag v-if="isExpired(row)" type="danger" size="small">已过期</el-tag>
            <el-tag v-else-if="row.enable" type="success" size="small">启用</el-tag>
            <el-tag v-else type="info" size="small">停用</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="过期时间" min-width="170">
          <template #default="{ row }">
            <span class="mono">{{ formatExpiry(row.expiredAt) }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="remark" label="备注" min-width="160">
          <template #default="{ row }">{{ row.remark || '-' }}</template>
        </el-table-column>
        <el-table-column prop="createTime" label="创建时间" min-width="180">
          <template #default="{ row }"><span class="mono">{{ row.createTime ?? '-' }}</span></template>
        </el-table-column>
        <el-table-column label="操作" width="300" fixed="right">
          <template #default="{ row }">
            <div class="row-actions">
              <el-button link type="primary" @click="openRemark(row)">备注</el-button>
              <el-button v-if="row.enable" link type="warning" @click="onDisable(row)">停用</el-button>
              <el-button v-else link type="success" @click="onEnable(row)">启用</el-button>
              <el-button link type="warning" @click="onReset(row)">重置密钥</el-button>
              <el-button link type="danger" @click="onDelete(row)">删除</el-button>
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

    <!-- 新增：真实密钥只在这次响应里出现，必须让操作者当场复制走。 -->
    <el-dialog v-model="addVisible" title="新增 API Key" width="520px">
      <el-form ref="addFormRef" :model="addForm" :rules="addRules" label-width="100px">
        <el-form-item label="应用标识" prop="app">
          <el-input v-model="addForm.app" placeholder="例如 partner-sy" />
        </el-form-item>
        <el-form-item label="归属用户">
          <el-input-number v-model="addForm.userId" :min="1" :controls="false" class="full" />
          <div class="tip">不填默认为 1（管理员账号）。</div>
        </el-form-item>
        <el-form-item label="过期时间">
          <el-input v-model="addForm.expiresAt" placeholder="YYYY-MM-DD HH:mm:ss，留空表示永不过期" />
          <div class="tip">留空 = 永不过期；过期记录会被后台每小时自动清理。</div>
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="addForm.remark" placeholder="用途说明" />
        </el-form-item>
        <el-form-item label="启用">
          <el-switch v-model="addForm.enable" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="addVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="onAddSubmit">创建</el-button>
      </template>
    </el-dialog>

    <!-- 真实密钥展示：只有新增/重置后才会出现一次。 -->
    <el-dialog v-model="secretVisible" title="请立即复制密钥" width="560px">
      <el-alert
        type="warning"
        :closable="false"
        show-icon
        title="完整密钥只显示这一次。关闭后无法再次查看，只能重置生成新的。"
        class="secret-tip"
      />
      <el-input :model-value="secretText" readonly class="mono secret-input">
        <template #append>
          <el-button @click="onCopySecret">复制</el-button>
        </template>
      </el-input>
      <template #footer>
        <el-button type="primary" @click="secretVisible = false">我已复制</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="remarkVisible" title="修改备注" width="420px">
      <el-form label-width="80px">
        <el-form-item label="备注">
          <el-input v-model="remarkText" placeholder="用途说明" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="remarkVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="onRemarkSubmit">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox, type FormInstance, type FormRules } from 'element-plus'
import {
  listApiKeys,
  addApiKey,
  setApiKeyRemark,
  enableApiKey,
  disableApiKey,
  resetApiKey,
  deleteApiKey,
  type ApiKeyItem
} from '@/api/apiKey'
import { getUserInfo } from '@/api/user'

const loading = ref(false)
/** 身份确认前不渲染表格，避免非管理员先看到一次 403 报错。 */
const authChecked = ref(false)
const isAdmin = ref(false)
const rows = ref<ApiKeyItem[]>([])
const router = useRouter()

const page = ref(1)
const count = ref(20)
const total = ref(0)

const addVisible = ref(false)
const saving = ref(false)
const addFormRef = ref<FormInstance>()
const addForm = reactive({
  app: '',
  userId: 1 as number | undefined,
  expiresAt: '',
  remark: '',
  enable: true
})
const addRules: FormRules = {
  app: [{ required: true, message: '请填写应用标识', trigger: 'blur' }]
}

const secretVisible = ref(false)
const secretText = ref('')

const remarkVisible = ref(false)
const remarkTarget = ref<ApiKeyItem | null>(null)
const remarkText = ref('')

/** 与后端 `authz::is_admin_role` 同口径：authority == '0' 视为管理员。 */
async function checkAuth() {
  try {
    const res = await getUserInfo()
    isAdmin.value = res.data?.role?.authority === '0'
  } catch {
    isAdmin.value = false
  } finally {
    authChecked.value = true
  }
}

async function loadData() {
  loading.value = true
  try {
    const res = await listApiKeys({ page: page.value, count: count.value })
    rows.value = res.data?.list ?? []
    total.value = res.data?.total ?? 0
  } catch (e: any) {
    ElMessage.error(e?.message ?? '加载 API Key 失败')
    rows.value = []
    total.value = 0
  } finally {
    loading.value = false
  }
}

/** 过期时间：Unix 秒 → 本地时间；null 表示永不过期。 */
function formatExpiry(expiredAt?: number | null) {
  if (expiredAt == null) return '永不过期'
  const d = new Date(expiredAt * 1000)
  if (Number.isNaN(d.getTime())) return '-'
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
}

function isExpired(row: any) {
  return row.expiredAt != null && row.expiredAt * 1000 < Date.now()
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

function openAdd() {
  addForm.app = ''
  addForm.userId = 1
  addForm.expiresAt = ''
  addForm.remark = ''
  addForm.enable = true
  addVisible.value = true
}

async function onAddSubmit() {
  try {
    await addFormRef.value?.validate()
  } catch {
    return
  }
  saving.value = true
  try {
    const res = await addApiKey({
      userId: addForm.userId,
      app: addForm.app.trim(),
      enable: addForm.enable,
      expiresAt: addForm.expiresAt.trim() || undefined,
      remark: addForm.remark.trim() || undefined
    })
    addVisible.value = false
    showSecret(res.data?.apiKey ?? '')
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '创建 API Key 失败')
  } finally {
    saving.value = false
  }
}

function showSecret(key: string) {
  secretText.value = key
  secretVisible.value = true
}

async function onCopySecret() {
  try {
    await navigator.clipboard.writeText(secretText.value)
    ElMessage.success('已复制到剪贴板')
  } catch {
    ElMessage.warning('复制失败，请手动选中复制')
  }
}

function openRemark(row: any) {
  remarkTarget.value = row
  remarkText.value = row.remark ?? ''
  remarkVisible.value = true
}

async function onRemarkSubmit() {
  const target = remarkTarget.value
  if (!target) return
  saving.value = true
  try {
    await setApiKeyRemark(target.id, remarkText.value.trim())
    remarkVisible.value = false
    ElMessage.success('备注已更新')
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '更新备注失败')
  } finally {
    saving.value = false
  }
}

async function onEnable(row: any) {
  try {
    await enableApiKey(row.id)
    ElMessage.success('已启用')
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '启用失败')
  }
}

async function onDisable(row: any) {
  try {
    await disableApiKey(row.id)
    ElMessage.success('已停用')
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '停用失败')
  }
}

async function onReset(row: any) {
  try {
    await ElMessageBox.confirm(
      '重置后旧密钥立即失效，正在使用旧密钥的调用方会全部收到 401。确定重置？',
      '重置密钥',
      { type: 'warning', confirmButtonText: '重置', cancelButtonText: '取消' }
    )
  } catch {
    return
  }
  try {
    const res = await resetApiKey(row.id)
    showSecret(res.data?.apiKey ?? '')
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '重置密钥失败')
  }
}

async function onDelete(row: any) {
  try {
    await ElMessageBox.confirm(
      `删除后该密钥立即失效且无法恢复（ID ${row.id}）。确定删除？`,
      '删除 API Key',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消' }
    )
  } catch {
    return
  }
  try {
    await deleteApiKey(row.id)
    ElMessage.success('已删除')
    loadData()
  } catch (e: any) {
    ElMessage.error(e?.message ?? '删除失败')
  }
}

onMounted(async () => {
  await checkAuth()
  if (isAdmin.value) loadData()
})
</script>

<style scoped>
.apikey-page {
  padding: 16px;
}

.page-header {
  margin-bottom: 12px;
}

.page-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.row-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.pager {
  margin-top: 12px;
  justify-content: flex-end;
}

.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}

.full {
  width: 100%;
}

.tip {
  color: var(--el-text-color-secondary);
  font-size: 12px;
  line-height: 1.5;
}

.secret-tip {
  margin-bottom: 12px;
}

.secret-input {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}
</style>
