<template>
  <div class="app-shell" :class="{ 'is-collapsed': !sidebar.opened, 'is-mobile-open': device === 'mobile' && sidebar.opened }">
    <div v-if="device==='mobile' && sidebar.opened" class="drawer-bg" @click="handleClickOutside" />
    <sidebar class="app-shell__sidebar" />
    <div class="app-shell__main">
      <navbar class="app-shell__navbar" />
      <tags-view class="app-shell__tags" />
      <app-main class="app-shell__content" />
    </div>
  </div>
</template>

<script>
import { Navbar, Sidebar, TagsView, AppMain } from './components'
import ResizeMixin from './mixin/ResizeHandler'

export default {
  name: 'Layout',
  components: { Navbar, Sidebar, TagsView, AppMain },
  mixins: [ResizeMixin],
  computed: {
    sidebar() { return this.$store.state.app.sidebar },
    device() { return this.$store.state.app.device }
  },
  methods: {
    handleClickOutside() {
      this.$store.dispatch('app/closeSideBar', { withoutAnimation: false })
    }
  }
}
</script>

<style lang="scss" scoped>
.app-shell {
  position: relative;
  display: flex;
  width: 100%;
  min-height: 100vh;
  background: var(--bg-base, #f5f8fc);

  &__sidebar {
    /* 侧栏：固定 220px 宽（不可被压缩） */
    flex: 0 0 220px;
    width: 220px;
    min-width: 220px;
    max-width: 220px;
    transition: width 0.24s ease, min-width 0.24s ease, max-width 0.24s ease, flex-basis 0.24s ease;
  }
  &__main {
    /* 主区：占据剩余全部空间 */
    flex: 1 1 auto;
    min-width: 0;
    width: auto;
    display: flex;
    flex-direction: column;
    background: var(--bg-base, #f5f8fc);
  }
  &__navbar {
    flex: 0 0 56px;
    min-height: 56px;
  }
  &__tags {
    flex: 0 0 38px;
    min-height: 38px;
  }
  &__content {
    flex: 1 1 auto;
    min-height: 0;
    width: 100%;
  }

  &.is-collapsed {
    .app-shell__sidebar {
      flex-basis: 64px;
      width: 64px;
      min-width: 64px;
      max-width: 64px;
    }
  }
}

@media (max-width: 1024px) {
  .app-shell {
    .app-shell__sidebar {
      flex-basis: 64px;
      width: 64px;
      min-width: 64px;
      max-width: 64px;
    }
    &.is-mobile-open {
      .app-shell__sidebar {
        flex-basis: 220px;
        width: 220px;
        min-width: 220px;
        max-width: 220px;
      }
    }
  }
}

.drawer-bg {
  position: absolute;
  inset: 0;
  background: rgba(15, 28, 45, 0.32);
  z-index: 25;
  backdrop-filter: blur(2px);
}
</style>
