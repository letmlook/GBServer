<template>
  <div v-if="isExternal" :style="styleExternalIcon" class="svg-external-icon svg-icon" />
  <svg v-else :class="svgClass" aria-hidden="true">
    <use :xlink:href="iconName" />
  </svg>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { isExternal as checkExternal } from '@/utils/validate'

const props = withDefaults(
  defineProps<{
    iconClass: string
    className?: string
  }>(),
  { className: '' }
)

const isExternal = computed(() => checkExternal(props.iconClass))
const iconName = computed(() => `#icon-${props.iconClass}`)
const svgClass = computed(() =>
  props.className ? `svg-icon ${props.className}` : 'svg-icon'
)
const styleExternalIcon = computed(() => ({
  mask: `url(${props.iconClass}) no-repeat 50% 50%`,
  WebkitMask: `url(${props.iconClass}) no-repeat 50% 50%`
}))
</script>

<style scoped>
.svg-icon {
  width: 1em;
  height: 1em;
  vertical-align: -0.15em;
  fill: currentColor;
  overflow: hidden;
}
.svg-external-icon {
  background-color: currentColor;
  mask-size: cover !important;
  display: inline-block;
}
</style>
