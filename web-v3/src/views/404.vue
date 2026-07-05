<template>
  <div class="app-404">
    <div class="num">404</div>
    <div class="msg">页面走丢了，去控制台看看？</div>
    <button class="gb-btn gb-btn--primary" @click="go">返回控制台</button>
  </div>
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router'
const router = useRouter()
const go = () => router.push('/')
</script>

<style lang="scss" scoped>
.app-404 {
  height: 100%;
  display: grid;
  place-content: center;
  text-align: center;
  gap: 12px;
  color: var(--text-secondary);
  .num { font-size: 96px; font-weight: 700; color: var(--brand-primary-500); font-family: var(--font-display); }
  .msg { font-size: var(--text-sm); }
}
</style>
