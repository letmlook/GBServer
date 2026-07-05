<template>
  <article class="gb-kpi" :style="containerStyle">
    <div class="gb-kpi__label">{{ label }}</div>
    <div class="gb-kpi__value" :style="{ color: valueColor }">
      {{ formattedValue }}
    </div>
    <div class="gb-kpi__trend" :style="{ color: trendColor }">
      <slot name="trend">{{ trend }}</slot>
    </div>
    <slot name="extra" />
    <svg
      v-if="sparkPoints"
      class="gb-kpi__spark"
      viewBox="0 0 200 38"
      preserveAspectRatio="none"
    >
      <path
        :d="`M0 38 L0 ${sparkPoints[0].y} ` + sparkPoints.map(p => `L ${p.x} ${p.y}`).join(' ') + ' L 200 38 Z'"
        :fill="sparkFill"
        opacity="0.20"
      />
      <path
        :d="'M0 ' + sparkPoints[0].y + ' ' + sparkPoints.map(p => `L ${p.x} ${p.y}`).join(' ')"
        :stroke="sparkStroke"
        stroke-width="1.5"
        fill="none"
      />
    </svg>
  </article>
</template>

<script setup lang="ts">
import { computed } from 'vue'

type TrendTone = 'success' | 'warning' | 'error' | 'neutral'
type ValueTone = 'success' | 'warning' | 'error' | 'primary' | 'default'

const props = withDefaults(
  defineProps<{
    label: string
    value?: string | number
    trend?: string
    trendTone?: TrendTone
    valueTone?: ValueTone
    spark?: number[] | null
    style?: Record<string, string>
  }>(),
  {
    value: '',
    trend: '',
    trendTone: 'neutral',
    valueTone: 'default',
    spark: null,
    style: () => ({})
  }
)

const TREND_COLOR: Record<TrendTone, string> = {
  success: 'var(--state-success)',
  warning: 'var(--state-warning)',
  error: 'var(--state-error)',
  neutral: 'var(--text-tertiary)'
}

const VALUE_COLOR: Record<ValueTone, string> = {
  success: 'var(--state-success)',
  warning: 'var(--state-warning)',
  error: 'var(--state-error)',
  primary: 'var(--brand-primary-500)',
  default: 'var(--text-primary)'
}

const SPARK_STROKE: Record<ValueTone, string> = {
  success: 'var(--state-success)',
  warning: 'var(--state-warning)',
  error: 'var(--state-error)',
  primary: 'var(--brand-primary-400)',
  default: 'var(--brand-primary-400)'
}

const formattedValue = computed(() => {
  if (typeof props.value === 'number') return props.value.toLocaleString('en-US')
  return props.value
})

const valueColor = computed(() => VALUE_COLOR[props.valueTone] || VALUE_COLOR.default)
const trendColor = computed(() => TREND_COLOR[props.trendTone] || TREND_COLOR.neutral)
const sparkStroke = computed(() => SPARK_STROKE[props.valueTone] || SPARK_STROKE.default)
const sparkFill = computed(() => `url(#spark-grad-${props.valueTone})`)
const containerStyle = computed(() => props.style)

const sparkPoints = computed(() => {
  if (!props.spark || props.spark.length < 2) return null
  const min = Math.min(...props.spark)
  const max = Math.max(...props.spark)
  const range = max - min || 1
  const w = 200
  const h = 38
  const step = w / (props.spark.length - 1)
  return props.spark.map((v, i) => ({
    x: i * step,
    y: h - ((v - min) / range) * (h - 4) - 2
  }))
})
</script>

<style lang="scss" scoped>
.gb-kpi {
  position: relative;
  background: var(--bg-surface);
  border: var(--layout-border);
  border-radius: var(--layout-radius);
  box-shadow: var(--shadow-card);
  padding: 16px 18px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  overflow: hidden;
  min-height: 116px;

  &__label {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    font-weight: 500;
  }
  &__value {
    font-family: var(--font-mono);
    font-size: var(--text-2xl);
    font-weight: 700;
    line-height: 1.2;
  }
  &__trend {
    font-size: var(--text-xs);
    min-height: 16px;
  }
  &__spark {
    position: absolute;
    inset: auto 0 0 0;
    width: 100%;
    height: 38px;
    pointer-events: none;
  }
}
</style>
