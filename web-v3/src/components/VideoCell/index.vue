<template>
  <div class="gb-video-cell" @click="emit('click')">
    <img v-if="thumb" :src="thumb" :alt="title || 'channel'" />
    <div v-else class="gb-video-cell__placeholder">{{ title || '视频' }}</div>
    <div class="gb-video-cell__overlay">
      <div class="gb-video-cell__overlay-top">
        <span :class="['gb-chip', 'gb-chip--' + chipTone]">{{ stateLabel }}</span>
        <span class="mono" style="font-size: 11px">{{ no }}</span>
      </div>
      <div class="gb-video-cell__overlay-bottom">{{ title }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

type State = 'live' | 'rec' | 'offline' | 'mute'

const props = withDefaults(
  defineProps<{
    title?: string
    no?: string | number
    state?: State
    thumb?: string
  }>(),
  { title: '', no: '', state: 'live', thumb: '' }
)

const emit = defineEmits<{ (e: 'click'): void }>()

const STATE_LABEL: Record<State, string> = {
  live: 'LIVE',
  rec: 'REC',
  offline: '离线',
  mute: '静音'
}

const stateLabel = computed(() => STATE_LABEL[props.state] || 'LIVE')
const chipTone = computed(() => (props.state === 'offline' ? 'mute' : props.state))
</script>

<style lang="scss" scoped>
.gb-video-cell {
  position: relative;
  aspect-ratio: 16 / 9;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--bg-elevated);
  cursor: pointer;
  transition: border-color 0.15s, transform 0.15s;

  &:hover { border-color: var(--brand-primary-300); }
  img { width: 100%; height: 100%; object-fit: cover; display: block; }

  &__placeholder {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    color: #7e8ea3;
    font-size: var(--text-xs);
  }
  &__overlay {
    position: absolute; inset: 0;
    display: flex; flex-direction: column;
    justify-content: space-between;
    padding: 6px 8px;
    background: linear-gradient(180deg, rgba(0, 0, 0, 0.32) 0%, transparent 30%, transparent 70%, rgba(0, 0, 0, 0.4) 100%);
    color: #fff;
    pointer-events: none;
  }
  &__overlay-top, &__overlay-bottom {
    display: flex; align-items: center; justify-content: space-between; gap: 6px;
    font-size: var(--text-xs);
  }
  &__overlay-top { color: #fff; }
  &__overlay-bottom { font-weight: 600; }
}
</style>
