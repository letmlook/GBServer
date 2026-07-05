<template>
  <article class="gb-kpi" :style="style">
    <div class="gb-kpi__label">{{ label }}</div>
    <div class="gb-kpi__value" :style="{ color: valueColor }">{{ formattedValue }}</div>
    <div class="gb-kpi__trend" :style="{ color: trendColor }">
      <slot name="trend">{{ trend }}</slot>
    </div>
    <slot name="extra" />
    <svg v-if="sparkPoints" class="gb-kpi__spark" viewBox="0 0 200 38" preserveAspectRatio="none">
      <path :d="`M0 38 L0 ${sparkPoints[0].y} ` + sparkPoints.map(p => `L ${p.x} ${p.y}`).join(' ') + ' L 200 38 Z'" :fill="sparkFill" opacity="0.20" />
      <path :d="'M0 ' + sparkPoints[0].y + ' ' + sparkPoints.map(p => `L ${p.x} ${p.y}`).join(' ')" :stroke="sparkStroke" stroke-width="1.5" fill="none" />
    </svg>
  </article>
</template>

<script>
export default {
  name: 'StatCard',
  props: {
    label: { type: String, required: true },
    value: { type: [String, Number], default: '' },
    trend: { type: String, default: '' },
    trendTone: { type: String, default: 'neutral' }, // success | warning | error | neutral
    valueTone: { type: String, default: 'default' }, // success | warning | error | default
    spark: { type: Array, default: () => null }, // number[]
    style: { type: Object, default: () => ({}) }
  },
  computed: {
    formattedValue() {
      if (typeof this.value === 'number') {
        return this.value.toLocaleString('en-US')
      }
      return this.value
    },
    valueColor() {
      return {
        success: 'var(--state-success)',
        warning: 'var(--state-warning)',
        error: 'var(--state-error)',
        primary: 'var(--brand-primary-500)',
        default: 'var(--text-primary)'
      }[this.valueTone] || 'var(--text-primary)'
    },
    trendColor() {
      return {
        success: 'var(--state-success)',
        warning: 'var(--state-warning)',
        error: 'var(--state-error)',
        neutral: 'var(--text-tertiary)'
      }[this.trendTone] || 'var(--text-tertiary)'
    },
    sparkPoints() {
      if (!this.spark || this.spark.length < 2) return null
      const min = Math.min(...this.spark)
      const max = Math.max(...this.spark)
      const range = max - min || 1
      const w = 200, h = 38
      const step = w / (this.spark.length - 1)
      return this.spark.map((v, i) => ({
        x: i * step,
        y: h - ((v - min) / range) * (h - 4) - 2
      }))
    },
    sparkStroke() {
      return {
        success: 'var(--state-success)',
        warning: 'var(--state-warning)',
        error: 'var(--state-error)',
        primary: 'var(--brand-primary-400)'
      }[this.valueTone] || 'var(--brand-primary-400)'
    },
    sparkFill() {
      return 'url(#spark-grad-' + this.valueTone + ')'
    }
  }
}
</script>
