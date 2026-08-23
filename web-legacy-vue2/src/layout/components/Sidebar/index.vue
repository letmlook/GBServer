<template>
  <aside class="app-sidebar" :class="{ 'is-collapsed': isCollapse }">
    <logo :collapse="isCollapse" />
    <nav class="app-nav" role="navigation" aria-label="主导航">
      <div v-for="group in groupedRoutes" :key="group.title" class="nav-group">
        <div v-if="!isCollapse" class="nav-group-title">{{ group.title }}</div>
        <router-link
          v-for="item in group.children"
          :key="item.fullPath"
          :to="item.fullPath"
          class="nav-item"
          :class="{ 'is-active': isActive(item) }"
        >
          <item :icon="(item.meta && item.meta.icon) || 'dashboard'" :title="item.meta && item.meta.title" />
          <span v-if="isLive(item) && !isCollapse" class="live-tag-mini">LIVE</span>
        </router-link>
      </div>
    </nav>
    <div v-if="!isCollapse" class="sidebar-foot">
      <div class="server-status-row">
        <span class="gb-dot gb-dot--success" />
        <span>SIP 网关 · 在线</span>
      </div>
      <div class="server-status-row">
        <span class="gb-dot gb-dot--success" />
        <span>媒体节点 ×14</span>
      </div>
      <div class="server-status-row">
        <span class="gb-dot gb-dot--warning" />
        <span>存储 78% 已用</span>
      </div>
    </div>
  </aside>
</template>

<script>
import Logo from './Logo'
import Item from './Item'

const GROUP_LABELS = {
  监控中心: ['控制台', 'Live', 'Playback', 'Map'],
  资源管理: ['Channel', 'Device', 'JTDevice', 'PushList', 'Proxy'],
  组织结构: ['Region', 'Group'],
  录像管理: ['RecordPlan', 'CloudRecord'],
  系统运维: ['Platform', 'MediaServer', 'User', 'OperationsSystemInfo', 'Alarm', 'OperationsHistoryLog', 'OperationsRealLog']
}

export default {
  name: 'Sidebar',
  components: { Logo, Item },
  data() {
    return { groupedRoutes: [] }
  },
  computed: {
    sidebar() { return this.$store.state.app.sidebar },
    device() { return this.$store.state.app.device },
    isCollapse() { return !this.sidebar.opened || this.device === 'mobile' }
  },
  watch: {
    $route: { handler: 'rebuild', immediate: true }
  },
  mounted() { this.rebuild() },
  methods: {
    rebuild() {
      const all = this.flattenRoutes(this.$router.options.routes)
      const groups = []
      const seen = new Set()
      Object.keys(GROUP_LABELS).forEach(title => {
        const children = GROUP_LABELS[title]
          .map(name => all.find(r => r.name === name))
          .filter(Boolean)
        if (children.length) {
          groups.push({ title, children })
          children.forEach(c => seen.add(c.name))
        }
      })
      const others = all.filter(r => !seen.has(r.name) && r.meta && r.meta.title)
      if (others.length) groups.push({ title: '其他', children: others })
      this.groupedRoutes = groups
    },
    flattenRoutes(routes, base) {
      const out = []
      base = base || '/'
      routes.forEach(r => {
        if (r.hidden || !r.meta || !r.meta.title) return
        if (r.children && r.children.length && !r.component && r.redirect !== 'noRedirect') {
          // 分组父级，跳过
        }
        const full = r.path && r.path.startsWith('/') ? r.path : (base.replace(/\/$/, '') + '/' + (r.path || ''))
        if (r.children && r.children.length) {
          out.push(...this.flattenRoutes(r.children, full))
        } else {
          out.push({ name: r.name, fullPath: full, meta: r.meta })
        }
      })
      return out
    },
    isActive(item) {
      const p = this.$route.path
      return p === item.fullPath || p.indexOf(item.fullPath + '/') === 0
    },
    isLive(item) {
      return item && item.meta && (item.meta.title === '分屏监控' || item.meta.title === '实时视频')
    }
  }
}
</script>
