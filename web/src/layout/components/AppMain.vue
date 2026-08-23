<template>
  <section class="app-main">
    <router-view v-slot="{ Component, route }">
      <transition name="fade-transform" mode="out-in">
        <keep-alive :include="cachedViewNames">
          <component :is="Component" :key="route.fullPath" />
        </keep-alive>
      </transition>
    </router-view>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useTagsViewStore } from '@/store/modules/tagsView'

const tagsView = useTagsViewStore()
const cachedViewNames = computed(() => tagsView.cachedViews)
</script>

<style scoped>
.app-main {
  flex: 1;
  width: 100%;
  position: relative;
  min-height: 0;
  overflow: auto;
}
</style>
