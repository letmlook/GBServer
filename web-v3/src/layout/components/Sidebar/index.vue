<template>
  <aside class="app-sidebar" :class="{ 'is-collapsed': isCollapse }">
    <Logo :collapse="isCollapse" />
    <nav class="app-nav" aria-label="主导航">
      <div v-for="group in groups" :key="group.title" class="nav-group">
        <div v-if="!isCollapse" class="nav-group-title">{{ group.title }}</div>
        <router-link
          v-for="item in group.children"
          :key="item.path"
          :to="item.path"
          class="nav-item"
          :class="{ 'is-active': isActive(item.path) }"
        >
          <svg-icon :icon-class="item.meta?.icon || 'dashboard'" class-name="nav-svg" />
          <span v-if="!isCollapse" class="nav-text">{{ item.meta?.title }}</span>
          <span v-if="!isCollapse && isLive(item)" class="live-tag-mini">LIVE</span>
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

<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter, type RouteRecordRaw } from 'vue-router'
import Logo from './Logo.vue'
import SvgIcon from '@/components/SvgIcon/index.vue'
import { useAppStore } from '@/store/modules/app'

const route = useRoute()
const router = useRouter()
const appStore = useAppStore()

const isCollapse = computed(
  () => !appStore.sidebar.opened || appStore.device === 'mobile'
)

const ROOT_GROUPS: Array<{ title: string; paths: string[] }> = [
  { title: '监控中心', paths: ['/dashboard'] },
  { title: '资源管理', paths: [] },
  { title: '运维中心', paths: [] }
]

interface NavItem {
  path: string
  name?: string
  meta?: { title?: string; icon?: string }
}

const groups = computed(() => {
  const all = flatten(router.options.routes as RouteRecordRaw[])
  return ROOT_GROUPS
    .map((g) => ({
      title: g.title,
      children: all.filter((r) => g.paths.includes(r.path))
    }))
    .filter((g) => g.children.length > 0)
})

function flatten(routes: RouteRecordRaw[], base = '/'): NavItem[] {
  const out: NavItem[] = []
  for (const r of routes) {
    if (r.meta?.hidden || !r.meta?.title) continue
    const full = r.path?.startsWith('/') ? r.path : `${base.replace(/\/$/, '')}/${r.path ?? ''}`
    if (r.children && r.children.length) {
      out.push(...flatten(r.children, full))
    } else {
      out.push({ path: full, name: r.name as string | undefined, meta: r.meta })
    }
  }
  return out
}

function isActive(p: string): boolean {
  return route.path === p || route.path.startsWith(p + '/')
}

function isLive(item: NavItem): boolean {
  return item.meta?.title === '分屏监控' || item.meta?.title === '实时视频'
}
</script>

<style lang="scss" scoped>
.app-sidebar {
  position: relative;
  background: var(--bg-surface);
  border-right: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
  width: 100%;
}

.app-nav { flex: 1; padding: 8px 6px; overflow-y: auto; }

.nav-group + .nav-group { margin-top: 14px; }

.nav-group-title {
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.5px;
  color: var(--text-tertiary);
  text-transform: uppercase;
  padding: 4px 10px 6px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 10px;
  height: 34px;
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.15s;
  position: relative;

  &:hover { background: var(--bg-hover); color: var(--brand-primary-500); }

  &.is-active {
    background: rgba(11, 138, 178, 0.10);
    color: var(--brand-primary-500);
    font-weight: 600;
  }
  .nav-svg { width: 16px; height: 16px; flex-shrink: 0; }
  .nav-text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
}

.live-tag-mini {
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
  color: #fff;
  background: var(--state-error);
  border-radius: 2px;
  padding: 1px 4px;
  letter-spacing: 0.5px;
}

.is-collapsed .nav-item { justify-content: center; padding: 0; }
.is-collapsed .live-tag-mini { display: none; }

.sidebar-foot {
  border-top: 1px solid var(--border-subtle);
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 10px;
  color: var(--text-tertiary);
}
.server-status-row { display: flex; align-items: center; gap: 6px; }
</style>
