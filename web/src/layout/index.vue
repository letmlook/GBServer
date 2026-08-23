<template>
  <div
    class="app-shell"
    :class="{
      'is-collapsed': !appStore.sidebar.opened,
      'is-mobile-open': appStore.device === 'mobile' && appStore.sidebar.opened
    }"
  >
    <div
      v-if="appStore.device === 'mobile' && appStore.sidebar.opened"
      class="drawer-bg"
      @click="appStore.closeSidebar(true)"
    />
    <Sidebar class="app-shell__sidebar" />
    <div class="app-shell__main">
      <Navbar class="app-shell__navbar" />
      <TagsView class="app-shell__tags" />
      <AppMain class="app-shell__content" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { useAppStore } from '@/store/modules/app'
import { useTagsViewSync } from '@/composables/useTagsViewSync'
import Sidebar from './components/Sidebar/index.vue'
import Navbar from './components/Navbar.vue'
import TagsView from './components/TagsView/index.vue'
import AppMain from './components/AppMain.vue'

const appStore = useAppStore()
useTagsViewSync()
</script>

<style lang="scss" scoped>
.app-shell {
  position: relative;
  display: flex;
  width: 100%;
  height: 100vh;
  overflow: hidden;
  background: var(--bg-base);

  &__sidebar {
    flex: 0 0 220px;
    width: 220px;
    min-width: 220px;
    max-width: 220px;
    transition:
      width 0.24s ease,
      min-width 0.24s ease,
      max-width 0.24s ease,
      flex-basis 0.24s ease;
  }
  &__main {
    flex: 1 1 auto;
    min-width: 0;
    width: auto;
    display: flex;
    flex-direction: column;
    background: var(--bg-base);
  }
  &__navbar { flex: 0 0 56px; min-height: 56px; }
  &__tags { flex: 0 0 38px; min-height: 38px; }
  &__content { flex: 1 1 auto; min-height: 0; }

  &.is-collapsed .app-shell__sidebar {
    flex-basis: 64px;
    width: 64px;
    min-width: 64px;
    max-width: 64px;
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
    &.is-mobile-open .app-shell__sidebar {
      flex-basis: 220px;
      width: 220px;
      min-width: 220px;
      max-width: 220px;
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
