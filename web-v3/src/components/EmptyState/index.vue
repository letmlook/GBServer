<template>
  <div class="gb-empty">
    <div class="gb-empty__icon">
      <svg viewBox="0 0 24 24" fill="none" width="40" height="40">
        <rect x="3" y="4" width="18" height="16" rx="2" stroke="currentColor" stroke-width="1.5" />
        <path d="M3 10h18" stroke="currentColor" stroke-width="1.5" />
        <path d="M8 14h8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
      </svg>
    </div>
    <div>{{ text || '暂无数据' }}</div>
    <slot />
  </div>
</template>

<script setup lang="ts">
withDefaults(defineProps<{ text?: string }>(), { text: '' })
</script>

<style lang="scss" scoped>
.gb-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 40px 16px;
  color: var(--text-tertiary);
  font-size: var(--text-xs);

  &__icon {
    color: var(--text-disabled);
  }
}
</style>
