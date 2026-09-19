<template>
  <!-- 抓图预览：
       - 默认以浮窗形式贴右下角，最新一张显示大图；下方时间线是最近抓过的图（点击切换）
       - 点击大图打开全屏对话框看原图，可下载或关闭 -->
  <div v-if="modelValue && items.length > 0" class="snap-floating" @click.stop>
    <div class="snap-floating__head">
      <span class="snap-floating__title">抓图预览 · {{ items.length }} 张</span>
      <el-button-group size="small">
        <el-button @click="openDialog" :icon="FullScreen">放大</el-button>
        <el-button @click="$emit('clear')" :icon="Delete">清空</el-button>
        <el-button @click="close" :icon="Close">收起</el-button>
      </el-button-group>
    </div>
    <el-image
      class="snap-floating__img"
      :src="items[0].snapUrl"
      :preview-src-list="items.map((i) => i.snapUrl)"
      :initial-index="0"
      fit="contain"
      @click="openDialog"
    />
    <div class="snap-floating__meta">
      <span class="mono">{{ items[0].deviceId }} / {{ items[0].channelId }}</span>
      <span class="text-tertiary">{{ items[0].name }}</span>
      <span class="text-tertiary text-xs">{{ formatTime(items[0].time) }}</span>
    </div>
    <div v-if="items.length > 1" class="snap-floating__thumbs">
      <el-image
        v-for="(it, i) in items.slice(0, 6)"
        :key="it.time"
        :src="it.snapUrl"
        :initial-index="i"
        :preview-src-list="items.map((x) => x.snapUrl)"
        class="snap-floating__thumb"
        fit="cover"
        @click="moveTo(i)"
      />
    </div>
  </div>

  <el-dialog
    v-model="dialogVisible"
    title="抓图"
    width="720px"
    :show-close="true"
    align-center
    destroy-on-close
  >
    <div v-if="dialogItem" class="snap-dialog">
      <el-image :src="dialogItem.snapUrl" fit="contain" class="snap-dialog__img" />
      <div class="snap-dialog__meta">
        <span class="mono">{{ dialogItem.deviceId }} / {{ dialogItem.channelId }}</span>
        <span>{{ dialogItem.name }}</span>
        <span class="text-tertiary text-xs">{{ formatTime(dialogItem.time) }}</span>
        <el-link :href="dialogItem.snapUrl" :underline="false" target="_blank" type="primary">在新窗口打开</el-link>
      </div>
    </div>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Close, Delete, FullScreen } from '@element-plus/icons-vue'

interface SnapItem {
  deviceId: string
  channelId: string
  name: string
  snapUrl: string
  time: number
}

const props = defineProps<{
  modelValue: boolean
  items: SnapItem[]
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'clear'): void
}>()

const dialogVisible = ref(false)
const dialogIndex = ref(0)

const dialogItem = computed<SnapItem | null>(() => props.items[dialogIndex.value] ?? null)

function close() {
  emit('update:modelValue', false)
}

function openDialog() {
  dialogIndex.value = 0
  dialogVisible.value = true
}

function moveTo(i: number) {
  dialogIndex.value = i
}

function formatTime(t: number): string {
  const d = new Date(t)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

// items 全部清空时自动隐藏浮窗
watch(
  () => props.items.length,
  (n) => {
    if (n === 0) emit('update:modelValue', false)
  }
)
</script>

<style lang="scss" scoped>
.snap-floating {
  position: fixed;
  right: 24px;
  bottom: 24px;
  width: 360px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
  padding: 10px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 6px;

  &__head {
    display: flex; justify-content: space-between; align-items: center;
    font-size: var(--text-xs); color: var(--text-secondary);
  }
  &__title { font-weight: 600; color: var(--text-primary); }
  &__img {
    width: 100%;
    aspect-ratio: 16 / 9;
    background: #000;
    border-radius: 6px;
    overflow: hidden;
    cursor: zoom-in;
    :deep(img) { width: 100%; height: 100%; object-fit: contain; }
  }
  &__meta {
    display: flex; flex-direction: column; gap: 2px;
    font-size: var(--text-sm);
  }
  &__thumbs {
    display: flex; gap: 6px; overflow-x: auto; padding-top: 4px;
  }
  &__thumb {
    flex: 0 0 60px; height: 40px;
    border-radius: 4px; overflow: hidden; background: #000;
    :deep(img) { width: 100%; height: 100%; object-fit: cover; }
  }
}

.snap-dialog {
  display: flex; flex-direction: column; gap: 8px;
  &__img {
    width: 100%;
    aspect-ratio: 16 / 9;
    background: #000;
    border-radius: 6px;
    overflow: hidden;
    :deep(img) { width: 100%; height: 100%; object-fit: contain; }
  }
  &__meta {
    display: flex; align-items: center; gap: 12px;
    font-size: var(--text-sm); flex-wrap: wrap;
  }
}
</style>