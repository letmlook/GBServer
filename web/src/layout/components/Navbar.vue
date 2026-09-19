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
      <!-- 平台信息：点击在按钮正下方弹出浮层，展示 SIP / JT1078 的对外接入
           参数（原来挂在控制台的「协议接入」卡片里，那里占一整张卡的位置，
           而这些参数属于"偶尔要抄一次"的配置，放顶栏随取随看更合适）。 -->
      <el-popover
        v-model:visible="platformVisible"
        placement="bottom-end"
        :width="420"
        trigger="click"
        popper-class="platform-info-popper"
      >
        <template #reference>
          <button
            class="topbar-platform"
            :class="{ 'is-active': platformVisible }"
            aria-label="平台信息"
          >
            <svg viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="9" stroke="currentColor" stroke-width="1.6" />
              <path d="M12 16v-4.5M12 8.2v.1" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
            </svg>
            <span>平台信息</span>
          </button>
        </template>

        <div class="platform-pop">
          <header class="platform-pop__head">
            <span class="platform-pop__title">接入协议信息</span>
            <span class="platform-pop__meta mono">{{ info?.host_ip ?? '-' }}</span>
          </header>
          <PlatformInfo
            :sip-config="info?.sip_config"
            :jt1078-config="info?.jt1078_config"
            :host-ip="info?.host_ip"
          />
        </div>
      </el-popover>

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
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { useAppStore } from '@/store/modules/app'
import { useUserStore } from '@/store/modules/user'
import { loadSystemInfo } from '@/utils/systemInfo'
import PlatformInfo from '@/components/PlatformInfo/index.vue'
import type { SystemInfo } from '@/api/log'

const appStore = useAppStore()
const userStore = useUserStore()
const route = useRoute()
const router = useRouter()

const query = ref('')

/**
 * 「平台信息」弹层的开关，以及后端 system/info 的最新一次响应。
 * 弹层里的 SIP / JT1078 接入参数直接取自这个响应。
 */
const platformVisible = ref(false)
const info = ref<SystemInfo | null>(null)
/** `system/info` 的轮询定时器（喂「平台信息」弹层） */
let infoTimer: number | null = null

/**
 * 周期性拉一次 `system/info`，喂给「平台信息」弹层里的
 * sip_config / jt1078_config / host_ip。
 *
 * 实际请求走 `@/utils/systemInfo`：导航栏、侧边栏、控制台会在挂载时同时拉这个接口，
 * 而它服务端要真采一次 CPU/网络（约 0.8s），各拉各的会让控制台首屏 CPU/内存/磁盘
 * 面板等到 2.5s。共享请求只合并"同一时刻的重复请求"，失败时返回 null，这里保留
 * 上一次的值即可（也不会弹 axios 的业务码 toast）。
 *
 * 注意：这个请求以前兼任「顶部导航栏延迟」的探针（用 performance.now()
 * 掐往返时间显示 `287ms 延迟`）。那个数字测得的是浏览器↔后端的 HTTP
 * 往返，而该接口服务端本身有固定 ~260ms 采样 sleep（CPU 60ms + 网络
 * 2×100ms），显示出来既不是网络延迟也不反映设备链路，已按要求去掉 ——
 * 需要看平台↔设备的真实延迟，用「国标设备」列表的「延迟」列。
 */
async function loadPlatformInfo() {
  const data = await loadSystemInfo()
  if (data) info.value = data
}

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

onMounted(() => {
  loadPlatformInfo()
  infoTimer = window.setInterval(loadPlatformInfo, 5_000)
})

onBeforeUnmount(() => {
  if (infoTimer !== null) {
    window.clearInterval(infoTimer)
    infoTimer = null
  }
})
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
  font-size: var(--text-xs);
  background: var(--bg-overlay);
  border-radius: 2px;
  padding: 1px 4px;
  color: var(--text-tertiary);
}

.topbar-right { display: flex; align-items: center; gap: 8px; margin-left: auto; }

/* 「平台信息」按钮：文字 + 问号图标，与右侧图标按钮同高 */
.topbar-platform {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 28px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
  background: var(--bg-surface);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  cursor: pointer;
  transition: color 0.15s, border-color 0.15s, background 0.15s;

  svg { width: 13px; height: 13px; }

  &:hover,
  &.is-active {
    color: var(--brand-primary-500);
    border-color: var(--brand-primary-300);
    background: rgba(11, 138, 178, 0.08);
  }
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
.user-role { font-size: var(--text-xs); color: var(--text-tertiary); }

/* Element Plus 的 popover 会被 teleport 到 body，scoped 选择器命中不到，
   所以弹层内部的样式写在全局（.platform-info-popper 作为命名空间）。 */
.platform-pop__head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
  padding-bottom: 10px;
  margin-bottom: 10px;
  border-bottom: 1px solid var(--border-subtle);
}
.platform-pop__title {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--text-primary);
}
.platform-pop__meta {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}
</style>

<style lang="scss">
/* popover 容器本身：去掉默认内边距，让上面的 head 贴边 */
.platform-info-popper.el-popover.el-popper {
  padding: 14px;
}
</style>
