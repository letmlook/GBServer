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

<script>
export default {
  name: 'SidebarLogo',
  props: {
    collapse: { type: Boolean, required: true }
  }
}
</script>
