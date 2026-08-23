<template>
  <el-pagination
    v-model:current-page="innerPage"
    v-model:page-size="innerSize"
    :total="total"
    :page-sizes="pageSizes"
    :layout="layout"
    :background="true"
    class="gb-pagination"
    @current-change="onPage"
    @size-change="onSize"
  />
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    page: number
    size: number
    total: number
    pageSizes?: number[]
    layout?: string
  }>(),
  {
    pageSizes: () => [20, 50, 100, 200],
    layout: 'total, sizes, prev, pager, next, jumper'
  }
)

const emit = defineEmits<{
  (e: 'update:page', v: number): void
  (e: 'update:size', v: number): void
  (e: 'change', page: number, size: number): void
}>()

const innerPage = computed({
  get: () => props.page,
  set: (v: number) => emit('update:page', v)
})

const innerSize = computed({
  get: () => props.size,
  set: (v: number) => emit('update:size', v)
})

function onPage(v: number) {
  emit('update:page', v)
  emit('change', v, innerSize.value)
}

function onSize(v: number) {
  emit('update:size', v)
  emit('change', innerPage.value, v)
}
</script>

<style scoped>
.gb-pagination { justify-content: flex-end; margin-top: 16px; }
</style>
