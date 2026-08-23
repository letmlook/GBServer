<template>
  <div class="gb-video-cell" @click="$emit('click')">
    <img v-if="thumb" :src="thumb" :alt="title || 'channel'">
    <div v-else class="gb-video-cell__placeholder">{{ title || '视频' }}</div>
    <div class="gb-video-cell__overlay">
      <div class="gb-video-cell__overlay-top">
        <span :class="['gb-chip', 'gb-chip--' + (state === 'offline' ? 'mute' : state)]">
          {{ stateLabel }}
        </span>
        <span class="mono" style="font-size:11px;">{{ no }}</span>
      </div>
      <div class="gb-video-cell__overlay-bottom">{{ title }}</div>
    </div>
  </div>
</template>

<script>
export default {
  name: 'VideoCell',
  props: {
    title: { type: String, default: '' },
    no: { type: [String, Number], default: '' },
    state: { type: String, default: 'live' }, // live | rec | offline | mute
    thumb: { type: String, default: '' }
  },
  computed: {
    stateLabel() {
      return { live: 'LIVE', rec: 'REC', offline: '离线', mute: '静音' }[this.state] || 'LIVE'
    }
  }
}
</script>

<style lang="scss" scoped>
.gb-video-cell__placeholder {
  width: 100%;
  height: 100%;
  display: grid;
  place-items: center;
  color: #7e8ea3;
  font-size: var(--text-xs);
}
</style>
