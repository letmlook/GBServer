<template>
  <el-card class="gb-table-card">
    <template v-if="title || $slots.header" #header>
      <div class="gb-table-header">
        <span>{{ title }}</span>
        <div class="flex-1" />
        <slot name="header" />
      </div>
    </template>
    <slot />
    <pagination
      v-if="total > 0"
      :page="page"
      :size="size"
      :total="total"
      @change="(p: number, s: number) => $emit('page-change', p, s)"
    />
  </el-card>
</template>

<script setup lang="ts">
import Pagination from '@/components/Pagination/index.vue'

defineProps<{
  title?: string
  page: number
  size: number
  total: number
}>()

defineEmits<{
  (e: 'page-change', page: number, size: number): void
}>()
</script>

<style scoped>
.gb-table-card { min-height: 400px; }
.gb-table-header { display: flex; align-items: center; gap: 12px; }
.flex-1 { flex: 1; }
</style>
