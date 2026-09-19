<template>
  <el-card class="gb-search-form">
    <el-form
      :inline="inline"
      :model="model"
      :class="{ 'gb-query-row': inline }"
      @submit.prevent="$emit('search')"
    >
      <slot :model="model" />
      <el-form-item>
        <el-button type="primary" @click="$emit('search')">查询</el-button>
        <el-button @click="onReset">重置</el-button>
      </el-form-item>
      <!-- 页面动作（刷新 / 新增 …）：与筛选条件同一行、靠右，替代原先页面顶部
           单独的一行按钮。 -->
      <el-form-item v-if="$slots.actions" class="gb-query-actions">
        <slot name="actions" :model="model" />
      </el-form-item>
    </el-form>
  </el-card>
</template>

<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    model: Record<string, unknown>
    inline?: boolean
  }>(),
  { inline: true }
)

const emit = defineEmits<{
  (e: 'search'): void
  (e: 'reset'): void
  (e: 'update:model', v: Record<string, unknown>): void
}>()

function onReset() {
  const cleared: Record<string, unknown> = {}
  for (const k of Object.keys(props.model)) cleared[k] = ''
  emit('update:model', cleared)
  emit('reset')
}
</script>

<style scoped>
.gb-search-form { margin-bottom: 12px; }
</style>
