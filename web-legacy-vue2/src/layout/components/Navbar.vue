<template>
  <header class="app-topbar">
    <div class="topbar-left">
      <button class="gb-icon-btn" :aria-label="sidebar.opened ? '折叠侧栏' : '展开侧栏'" @click="toggleSideBar">
        <svg v-if="sidebar.opened" viewBox="0 0 24 24" fill="none">
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
        >
        <span class="kbd">/</span>
      </div>
    </div>

    <div class="topbar-right">
      <div class="topbar-stat" v-if="latency">
        <span class="gb-dot gb-dot--success" />
        <span class="mono">{{ latency }}</span>
        <span class="text-tertiary">延迟</span>
      </div>
      <button class="gb-icon-btn" aria-label="告警" @click="goToAlarm">
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
          <div>
            <div class="user-name">{{ name || 'admin' }}</div>
            <div class="user-role">{{ role || '超级管理员' }}</div>
          </div>
        </div>
        <el-dropdown-menu slot="dropdown">
          <el-dropdown-item icon="el-icon-key" command="password">修改密码</el-dropdown-item>
          <el-dropdown-item icon="el-icon-setting" command="profile">个人设置</el-dropdown-item>
          <el-dropdown-item divided icon="el-icon-switch-button" command="logout">注销</el-dropdown-item>
        </el-dropdown-menu>
      </el-dropdown>
    </div>

    <changePasswordDialog ref="changePasswordDialog" />
  </header>
</template>

<script>
import { mapGetters } from 'vuex'
import changePasswordDialog from './dialog/changePassword.vue'

export default {
  name: 'Navbar',
  components: { changePasswordDialog },
  data() {
    return { query: '', latency: '12ms' }
  },
  computed: {
    ...mapGetters(['sidebar', 'name', 'role']),
    parentTitle() {
      const matched = this.$route.matched
      if (matched.length < 2) return ''
      const parent = matched[matched.length - 2]
      return (parent.meta && parent.meta.title) || ''
    },
    currentTitle() {
      return (this.$route.meta && this.$route.meta.title) || ''
    },
    avatarLetter() {
      return (this.name || 'A').slice(0, 1).toUpperCase()
    }
  },
  methods: {
    toggleSideBar() { this.$store.dispatch('app/toggleSideBar') },
    toggleTheme() {
      this.$message && this.$message.info('主题切换：预留接口')
    },
    onSearch() {
      if (!this.query) return
      this.$message && this.$message.info(`搜索：${this.query}`)
    },
    goToAlarm() { this.$router.push('/alarm') },
    async onCommand(cmd) {
      if (cmd === 'logout') {
        await this.$store.dispatch('user/logout')
        this.$router.push(`/login?redirect=${this.$route.fullPath}`)
      } else if (cmd === 'password') {
        this.$refs.changePasswordDialog.openDialog(this.logout)
      } else if (cmd === 'profile') {
        this.$message && this.$message.info('个人设置：预留页面')
      }
    },
    bindGlobalKey(e) {
      if (e.key === '/' && document.activeElement && document.activeElement.tagName !== 'INPUT' && document.activeElement.tagName !== 'TEXTAREA') {
        e.preventDefault()
        this.$el.querySelector('.gb-search input') && this.$el.querySelector('.gb-search input').focus()
      }
    }
  },
  mounted() {
    document.addEventListener('keydown', this.bindGlobalKey)
  },
  beforeDestroy() {
    document.removeEventListener('keydown', this.bindGlobalKey)
  }
}
</script>

<style scoped>
.app-topbar { background: var(--bg-surface); }
</style>
