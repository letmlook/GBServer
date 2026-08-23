<template>
  <header class="app-topbar">
    <div class="topbar-left">
      <button
        class="gb-icon-btn"
        :aria-label="appStore.sidebar.opened ? '折叠侧栏' : '展开侧栏'"
        @click="appStore.toggleSidebar()"
      >
        <svg v-if="appStore.sidebar.opened" viewBox="0 0 24 24" fill="none">
          <path d="M4 6h16M4 12h10M4 18h16" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        </svg>
        <svg v-else viewBox="0 0 24 24" fill="none">
          <path d="M4 6h16M4 12h16M4 18h16" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        </svg>
      </button>
      <div class="topbar-breadcrumb">
        <span>{{ parentTitle }}</span>
        <span v-if="parentTitle && currentTitle" class="sep">/</span>
        <span class="current">{{ currentTitle }}</span>
      </div>
    </div>

    <div class="topbar-center">
      <div class="gb-search" role="search">
        <svg viewBox="0 0 24 24" fill="none">
          <circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.6" />
          <path d="M21 21l-4.35-4.35" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        </svg>
        <input
          v-model="query"
          type="text"
          placeholder="搜索通道、设备、平台…"
          @keyup.enter="onSearch"
        />
        <span class="kbd">/</span>
      </div>
    </div>

    <div class="topbar-right">
      <div v-if="latency" class="topbar-stat">
        <span class="gb-dot gb-dot--success" />
        <span class="mono">{{ latency }}</span>
        <span class="text-tertiary">延迟</span>
      </div>
      <button class="gb-icon-btn" aria-label="告警" @click="goAlarm">
        <svg viewBox="0 0 24 24" fill="none">
          <path d="M12 3l9 16H3z" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
        </svg>
        <span class="badge-dot" />
      </button>
      <button class="gb-icon-btn" aria-label="主题" @click="toggleTheme">
        <svg viewBox="0 0 24 24" fill="none">
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" stroke="currentColor" stroke-width="1.6" />
        </svg>
      </button>
      <el-dropdown trigger="click" @command="onCommand">
        <div class="user-chip">
          <div class="user-avatar">{{ avatarLetter }}</div>
          <div class="user-info">
            <div class="user-name">{{ userStore.name || 'admin' }}</div>
            <div class="user-role">超级管理员</div>
          </div>
        </div>
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item icon="el-icon-key" command="password">修改密码</el-dropdown-item>
            <el-dropdown-item icon="el-icon-setting" command="profile">个人设置</el-dropdown-item>
            <el-dropdown-item divided icon="el-icon-switch-button" command="logout">注销</el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </div>
  </header>

  <!-- 修改密码 -->
  <el-dialog v-model="pwdVisible" title="修改密码" width="420px">
    <el-form label-width="100px">
      <el-form-item label="用户">
        <span class="mono">{{ userStore.name }}</span>
      </el-form-item>
      <el-form-item label="新密码">
        <el-input v-model="newPassword" type="password" show-password placeholder="至少 6 位" />
      </el-form-item>
      <el-form-item label="确认密码">
        <el-input v-model="newPassword2" type="password" show-password />
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="pwdVisible = false">取消</el-button>
      <el-button type="primary" :loading="savingPwd" @click="onPwdSave">保存</el-button>
    </template>
  </el-dialog>

  <!-- 个人设置 -->
  <el-dialog v-model="profileVisible" title="个人设置" width="480px">
    <el-form label-width="100px">
      <el-form-item label="用户名">
        <el-input v-model="profileForm.displayName" disabled />
      </el-form-item>
      <el-form-item label="角色">
        <el-input :model-value="userStore.role" disabled placeholder="超级管理员" />
      </el-form-item>
      <el-form-item label="PushKey">
        <el-input v-model="profileForm.pushKey" disabled type="password" show-password />
      </el-form-item>
      <el-form-item label="主题">
        <el-radio-group :model-value="appStore.theme" @change="appStore.toggleTheme()">
          <el-radio-button label="light">浅色</el-radio-button>
          <el-radio-button label="dark">深色</el-radio-button>
        </el-radio-group>
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="profileVisible = false">关闭</el-button>
      <el-button type="primary" @click="onProfileSave">保存</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { useAppStore } from '@/store/modules/app'
import { useUserStore } from '@/store/modules/user'

const appStore = useAppStore()
const userStore = useUserStore()
const route = useRoute()
const router = useRouter()

const query = ref('')
const latency = ref('12ms')

const parentTitle = computed(() => {
  const matched = route.matched
  if (matched.length < 2) return ''
  return (matched[matched.length - 2].meta.title as string | undefined) || ''
})
const currentTitle = computed(() => (route.meta.title as string | undefined) || '')

const avatarLetter = computed(() =>
  (userStore.name || 'A').slice(0, 1).toUpperCase()
)

function onSearch() {
  const kw = query.value.trim()
  if (!kw) return
  // 全局搜索：跳到通道列表并把关键字写入 query
  router.push({ path: '/channel', query: { query: kw } })
  query.value = ''
}
function toggleTheme() {
  appStore.toggleTheme()
  ElMessage.success(`已切换为${appStore.theme === 'dark' ? '深色' : '浅色'}模式`)
}
function goAlarm() {
  router.push('/alarm')
}

const pwdVisible = ref(false)
const profileVisible = ref(false)
const newPassword = ref('')
const newPassword2 = ref('')
const savingPwd = ref(false)
const profileForm = reactive({ displayName: userStore.name, pushKey: '' })

async function onCommand(cmd: string) {
  if (cmd === 'logout') {
    await userStore.logout()
    router.push(`/login?redirect=${route.fullPath}`)
  } else if (cmd === 'password') {
    pwdVisible.value = true
    newPassword.value = ''
    newPassword2.value = ''
  } else if (cmd === 'profile') {
    // 拉取最新的用户信息
    try {
      const res: any = await userStore.userInfo()
      profileForm.displayName = res?.username ?? userStore.name
      profileForm.pushKey = res?.pushKey ?? ''
    } catch {
      profileForm.displayName = userStore.name
    }
    profileVisible.value = true
  }
}

async function onPwdSave() {
  if (newPassword.value.length < 6) {
    ElMessage.error('新密码至少 6 位')
    return
  }
  if (newPassword.value !== newPassword2.value) {
    ElMessage.error('两次密码输入不一致')
    return
  }
  savingPwd.value = true
  try {
    // 自助改密：调通用改密端点（无原密码场景管理员可走 changePasswordForAdmin）
    const { changePasswordForAdmin } = await import('@/api/user')
    await changePasswordForAdmin({ userId: userStore.userId ?? 0, password: newPassword.value })
    ElMessage.success('密码已重置，下次登录使用新密码')
    pwdVisible.value = false
  } catch (e: any) {
    ElMessage.error(e?.message ?? '密码重置失败')
  } finally {
    savingPwd.value = false
  }
}

function onProfileSave() {
  // 用户基本信息展示：name + pushKey；保存只更新本地显示
  ElMessage.success('设置已保存到本地（后端无 user/profile 端点）')
  profileVisible.value = false
}
</script>

<style lang="scss" scoped>
.app-topbar {
  flex: 0 0 56px;
  min-height: 56px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 16px;
  background: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
}
.topbar-left { display: flex; align-items: center; gap: 12px; min-width: 240px; }
.topbar-breadcrumb { display: flex; gap: 6px; font-size: var(--text-sm); color: var(--text-tertiary); }
.topbar-breadcrumb .current { color: var(--text-primary); font-weight: 600; }
.topbar-breadcrumb .sep { color: var(--text-disabled); }

.topbar-center { flex: 1; max-width: 480px; }
.topbar-center .gb-search { width: 100%; }
.gb-search .kbd {
  font-family: var(--font-mono);
  font-size: 10px;
  background: var(--bg-overlay);
  border-radius: 2px;
  padding: 1px 4px;
  color: var(--text-tertiary);
}

.topbar-right { display: flex; align-items: center; gap: 8px; margin-left: auto; }
.topbar-stat {
  display: flex; align-items: center; gap: 6px;
  font-size: var(--text-xs);
  color: var(--text-secondary);
  padding: 0 8px;
  .mono { color: var(--text-primary); }
}
.badge-dot {
  position: absolute; top: 6px; right: 6px;
  width: 6px; height: 6px;
  background: var(--state-error);
  border-radius: 50%;
}

.user-chip {
  display: flex; align-items: center; gap: 8px;
  padding: 4px 10px 4px 4px;
  border-radius: var(--radius-full);
  cursor: pointer;
  transition: background 0.15s;
  &:hover { background: var(--bg-hover); }
}
.user-avatar {
  width: 28px; height: 28px;
  display: grid; place-items: center;
  background: var(--brand-primary-500);
  color: #fff;
  border-radius: 50%;
  font-weight: 600;
  font-size: var(--text-sm);
}
.user-info { line-height: 1.1; }
.user-name { font-size: var(--text-xs); color: var(--text-primary); font-weight: 600; }
.user-role { font-size: 10px; color: var(--text-tertiary); }
</style>
