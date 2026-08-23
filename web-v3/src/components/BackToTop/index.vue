<template>
  <transition name="fade">
    <button v-if="visible" class="back-to-top" @click="onClick" aria-label="回到顶部">
      <el-icon size="20"><ArrowUp /></el-icon>
    </button>
  </transition>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { ArrowUp } from '@element-plus/icons-vue'

const visible = ref(false)
const threshold = 400

function onScroll() {
  visible.value = window.scrollY > threshold
}

function onClick() {
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

onMounted(() => window.addEventListener('scroll', onScroll, { passive: true }))
onUnmounted(() => window.removeEventListener('scroll', onScroll))
</script>

<style scoped>
.back-to-top {
  position: fixed;
  right: 32px;
  bottom: 32px;
  width: 44px;
  height: 44px;
  border-radius: 50%;
  border: none;
  background: var(--el-color-primary, #0b8ab2);
  color: #fff;
  cursor: pointer;
  box-shadow: 0 4px 14px rgba(11, 138, 178, 0.4);
  z-index: 99;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 0.2s;
}
.back-to-top:hover { transform: translateY(-3px); }
.fade-enter-active, .fade-leave-active { transition: opacity 0.2s; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
</style>
