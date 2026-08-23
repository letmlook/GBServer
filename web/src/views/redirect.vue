<template>
  <div />
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'

const route = useRoute()
const router = useRouter()

onMounted(() => {
  // 路由模式: /redirect/<original path with / encoded>
  // 例: /redirect/dashboard  →  跳 /dashboard
  // 例: /redirect/live?deviceId=xxx&channelId=xxx  →  跳 /live?...
  const raw = (route.params.path as string) ?? ''
  // path: '*' 模式下 params.path 包含子路径（不含 leading /），补上
  const target = '/' + raw + (route.hash || '')
  router.replace(target)
})
</script>
