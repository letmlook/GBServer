<template>
  <!-- 整个控件外观是一个输入框（与 EP 的日期范围编辑器一致）：
       日期和起止时刻都在**同一个框里**，点日期开日历、点时刻开下拉，
       不再有并排的独立时间下拉框。 -->
  <div
    class="gb-dt-range"
    :class="{ 'is-open': opened, 'is-empty': !dates }"
    @click="openCalendar"
  >
    <el-icon class="gb-dt-range__icon"><Calendar /></el-icon>

    <span class="gb-dt-range__date">{{ dates?.[0] ?? '开始日期' }}</span>
    <GbTimeSelect
      v-model="startClock"
      class="gb-dt-range__time"
      :step="step"
      placeholder="开始时间"
      @click.stop
    />

    <span class="gb-dt-range__sep">~</span>

    <span class="gb-dt-range__date">{{ dates?.[1] ?? '结束日期' }}</span>
    <!-- 结束时间多一档 24:00（当天结束 = 次日 00:00），开始时间不需要 -->
    <GbTimeSelect
      v-model="endClock"
      class="gb-dt-range__time"
      :step="step"
      include-day-end
      placeholder="结束时间"
      @click.stop
    />

    <!-- 常驻占位：有值时才显示，避免选中/清空时整块控件宽度来回跳 -->
    <el-icon
      class="gb-dt-range__clear"
      :class="{ 'is-hidden': !dates }"
      @click.stop="dates && clear()"
    >
      <CircleClose />
    </el-icon>

    <!-- 真实的日期范围选择器：透明叠在整块控件上，只用它的日历弹层。
         （不直接用它的输入框，是因为弹层里的时间只能靠滚轮改，
         而时间要能和日期并排放在同一个框里直接点选。） -->
    <el-date-picker
      ref="pickerRef"
      v-model="dates"
      class="gb-dt-range__picker"
      type="daterange"
      range-separator="~"
      value-format="YYYY-MM-DD"
      :clearable="false"
      @visible-change="(v: boolean) => (opened = v)"
    />
  </div>
</template>

<script setup lang="ts">
/**
 * 日期范围 + 起止时刻（查询条件用），外观是**一个输入框**：
 *
 *     [📅 2026-09-10  08:00 ▾  ~  2026-09-20  18:00 ▾   ⊗]
 *
 * - 点日期文字 → 弹出日历选日期范围；
 * - 点时刻 → 弹出 15 分钟档的下拉（日期与时间在同一个框里，不用先在别处
 *   单独选时间）；
 * - 时刻下拉本身是 `GbTimeSelect`（不用 `el-time-select` 的原因见该组件：
 *   它选不出 24:00）。
 *
 * v-model 是 `[Date, Date] | null`（本地时间的完整时间戳，两端都含）：
 * - 日期范围为空 → `null`（表示不加时间过滤，与改造前各页面语义一致）；
 * - 结束时间选 `24:00` → 归一化成**次日 00:00**，这样「当天 24:00」不会
 *   丢掉 23:45~24:00 这一段；
 * - 反向显示时若结束时刻正好是 00:00:00 且结束日期晚于开始日期，则回显成
 *   「前一天 24:00」，与上面的归一化互逆，来回切换不会漂移。
 *
 * 「开始 18:00 / 结束 00:00 同一天」这种结束早于开始的组合不会报错也不清空，
 * 而是把结束理解成**开始之后的最近一次**该时刻（跨到次日）——否则同一个
 * 日期上起止顺序一反，用户会查到一个空列表却看不出原因。
 */
import { ref, watch } from 'vue'
import { Calendar, CircleClose } from '@element-plus/icons-vue'
import GbTimeSelect from '@/components/GbTimeSelect/index.vue'

const START = '00:00'
/** 「当天结束」的哨兵档位：combine() 会把它归一到次日 00:00 */
const END = '24:00'

const props = withDefaults(
  defineProps<{
    modelValue?: [Date, Date] | null
    /** 时间下拉的档位，默认 15 分钟（与「录像计划」一致） */
    step?: string
  }>(),
  {
    modelValue: null,
    step: '00:15'
  }
)

const emit = defineEmits<{
  (e: 'update:modelValue', value: [Date, Date] | null): void
}>()

/** 叠在控件上的日期选择器（只借它的日历弹层）——EP 通过 expose 提供 handleOpen */
const pickerRef = ref<{ handleOpen: () => void } | null>(null)
const opened = ref(false)
const openCalendar = () => pickerRef.value?.handleOpen()

/** `el-date-picker` 用 value-format，内部存 'YYYY-MM-DD' 字符串 */
const dates = ref<[string, string] | null>(null)
const startClock = ref(START)
const endClock = ref(END)

const pad = (n: number) => String(n).padStart(2, '0')
const toDateStr = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
const toClock = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}`

/** 'YYYY-MM-DD' + 'HH:mm' → 本地 Date（'24:00' 归一到次日 00:00） */
function combine(dateStr: string, clock: string): Date {
  const [y, m, d] = dateStr.split('-').map(Number)
  const [hh, mm] = clock.split(':').map(Number)
  return new Date(y, m - 1, d, hh, mm, 0, 0)
}

/** 当前控件状态对应的值（没有日期范围时是 null） */
function current(): [Date, Date] | null {
  if (!dates.value) return null
  const start = combine(dates.value[0], startClock.value)
  const end = combine(dates.value[1], endClock.value)
  // 结束早于开始 → 按「开始之后最近一次该时刻」解释（跨到次日）
  const fixedEnd = end.getTime() < start.getTime() ? new Date(end.getTime() + 86400_000) : end
  return [start, fixedEnd]
}

const sameValue = (a: [Date, Date] | null, b: [Date, Date] | null) =>
  a === b || (!!a && !!b && a[0].getTime() === b[0].getTime() && a[1].getTime() === b[1].getTime())

function clear() {
  dates.value = null
  startClock.value = START
  endClock.value = END
}

// 外部值 → 控件状态（初值、重置、父组件回写）
watch(
  () => props.modelValue,
  (v) => {
    // 自己刚 emit 出去的值会被父组件回灌，直接跳过：
    // 否则「结束 24:00」会被重新解释成「次日 00:00」，界面上的结束日期
    // 会自己往后跳一天。
    if (sameValue(v, current())) return
    if (!v) {
      clear()
      return
    }
    const [start, end] = v
    let endDate = end
    let endClockText = toClock(end)
    // 结束时刻是整点 00:00 且晚于开始日期 → 回显成前一天 24:00（更贴近
    // 「整天」的直觉，且与 current() 的归一化互为逆运算）
    if (end.getHours() === 0 && end.getMinutes() === 0 && toDateStr(end) !== toDateStr(start)) {
      endDate = new Date(end.getTime() - 86400_000)
      endClockText = END
    }
    dates.value = [toDateStr(start), toDateStr(endDate)]
    startClock.value = toClock(start)
    endClock.value = endClockText
  },
  { immediate: true }
)

// 控件状态 → 外部值
watch([dates, startClock, endClock], () => {
  const next = current()
  if (!sameValue(next, props.modelValue)) emit('update:modelValue', next)
})
</script>

<style scoped>
/* 外观对齐 Element Plus 的 `.el-range-editor`，让整块看起来就是一个输入框 */
.gb-dt-range {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 2px;
  /* 宽度由内容撑开（各段都是不收缩的固定/弹性尺寸），
     写死宽度会把日期压成省略号 */
  width: auto;
  height: 32px;
  padding: 0 8px 0 10px;
  background: var(--bg-surface);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: border-color 0.15s, box-shadow 0.15s;
}
.gb-dt-range:hover {
  border-color: var(--brand-primary-300);
}
.gb-dt-range.is-open {
  border-color: var(--brand-primary-300);
  box-shadow: 0 0 0 2px rgba(11, 138, 178, 0.12);
}

.gb-dt-range__icon {
  flex: none;
  color: var(--text-tertiary);
  font-size: 14px;
}

/* 固定宽度：YYYY-MM-DD 放得下，且空/非空两种状态的宽度一致（不会有跳动） */
.gb-dt-range__date {
  flex: none;
  width: 92px;
  overflow: hidden;
  text-align: center;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--text-primary);
}
.gb-dt-range.is-empty .gb-dt-range__date {
  color: var(--text-disabled);
}

.gb-dt-range__sep {
  flex: none;
  color: var(--text-tertiary);
}

/* 时刻下拉：去掉 el-select 自带的边框/底色，融进同一个框里。
   宽度要给足：78px 时「文字 + 箭头」正好把内容盒占满，文字会被
   压出省略号（显示成「00:...」）。 */
.gb-dt-range :deep(.gb-time-select) {
  flex: none;
  width: 92px;
}
.gb-dt-range :deep(.gb-time-select .el-select__wrapper) {
  min-height: 24px;
  padding: 0 18px 0 4px;
  background: transparent;
  box-shadow: none;
  font-size: var(--text-sm);
}
.gb-dt-range :deep(.gb-time-select .el-select__wrapper:hover) {
  background: var(--bg-hover);
  border-radius: var(--radius-sm);
}
.gb-dt-range :deep(.gb-time-select .el-select__caret) {
  color: var(--text-tertiary);
}

.gb-dt-range__clear {
  flex: none;
  color: var(--text-tertiary);
  font-size: 14px;
  cursor: pointer;
}
.gb-dt-range__clear:hover {
  color: var(--text-secondary);
}
.gb-dt-range__clear.is-hidden {
  visibility: hidden;
}

/* 只借日历弹层的真实日期选择器：透明铺满整块控件，
   这样弹层的位置正好贴着控件下沿。
   注意必须走 `:deep()`：`el-date-picker` 的根节点带的是 EP 自己的类，
   拿不到本组件的 `data-v-*` 属性（实测该节点只有 class/tabindex），
   普通 scoped 选择器对它**完全不生效**——漏了这层，隐藏的选择器
   会照常显示并占掉半个控件的宽度。 */
.gb-dt-range :deep(.gb-dt-range__picker) {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  opacity: 0;
  pointer-events: none;
}
</style>
