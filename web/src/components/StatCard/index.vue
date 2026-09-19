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
    style?: Record<string, string>
  }>(),
  {
    value: '',
    trend: '',
    trendTone: 'neutral',
    valueTone: 'default',
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

// spark 之前用 mem / net 历史给卡片画"小曲线"，但**这些数据与 KPI
// 数值毫无业务关联**，容易让用户误以为是"在线设备变化趋势"。
// 现已从前端彻底移除：组件不再接受 spark prop，不再画 SVG 曲线。

/**
 * 下面这几个派生值**必须是 computed**。
 *
 * 移除 spark 时曾把它们简化成立即求值的常量（`(() => ...)()`），
 * 结果是在 setup 阶段只算一次、拿到的是 props 的**初始值**
 * （数字是 0、tone 是默认档），之后数据到位也不再重算 ——
 * 表现就是控制台顶部 4 个卡片永远显示 0。
 *
 * props 是响应式的，任何依赖 props 的值都得走 computed 才能跟着更新。
 */
const formattedValue = computed(() =>
  typeof props.value === 'number' ? props.value.toLocaleString('en-US') : props.value
)
const valueColor = computed(() => VALUE_COLOR[props.valueTone] || VALUE_COLOR.default)
const trendColor = computed(() => TREND_COLOR[props.trendTone] || TREND_COLOR.neutral)
const containerStyle = computed(() => props.style)
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
}
</style>
