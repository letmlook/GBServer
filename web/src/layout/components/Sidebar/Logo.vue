<template>
  <div class="app-logo">
    <div class="logo-mark">
      <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <rect x="2.5" y="2.5" width="19" height="19" rx="3" stroke="currentColor" stroke-width="1.5" />
        <rect x="7" y="7" width="10" height="10" rx="1.5" stroke="currentColor" stroke-width="1.2" opacity="0.5" />
        <circle cx="12" cy="12" r="2.4" fill="currentColor" />
        <line x1="12" y1="1" x2="12" y2="3.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
        <line x1="12" y1="20.5" x2="12" y2="23" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
        <line x1="1" y1="12" x2="3.5" y2="12" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
        <line x1="20.5" y1="12" x2="23" y2="12" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
      </svg>
    </div>
    <div v-if="!collapse" class="logo-text">
      <div class="logo-name">GBServer</div>
      <div class="logo-sub">视频融合平台</div>
    </div>
    <span v-if="!collapse" class="logo-pulse">LIVE</span>
  </div>
</template>

<script setup lang="ts">
defineProps<{ collapse: boolean }>()
</script>

<style lang="scss" scoped>
.app-logo {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--border-subtle);
  color: var(--brand-primary-500);
  position: relative;
}
.logo-mark {
  width: 32px; height: 32px;
  display: grid; place-items: center;
  background: rgba(11, 138, 178, 0.08);
  border-radius: var(--radius-sm);
  flex-shrink: 0;
  svg { width: 18px; height: 18px; }
}
.logo-text { line-height: 1.2; min-width: 0; }
.logo-name { font-size: var(--text-sm); font-weight: 700; color: var(--text-primary); }
.logo-sub  { font-size: 10px; color: var(--text-tertiary); }
.logo-pulse {
  margin-left: auto;
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
  color: var(--state-error);
  background: rgba(220, 38, 38, 0.10);
  border-radius: 2px;
  padding: 2px 4px;
  letter-spacing: 0.5px;
}
</style>
