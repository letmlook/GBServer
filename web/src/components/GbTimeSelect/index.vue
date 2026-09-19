<template>
  <el-select v-model="value" :placeholder="placeholder" class="gb-time-select">
    <el-option v-for="c in clocks" :key="c" :label="c" :value="c" />
    <!-- 「当天结束」：选它等价于次日 00:00，这样按天查询不会丢掉 23:45~24:00 -->
    <el-option v-if="includeDayEnd" label="24:00" value="24:00" />
  </el-select>
</template>

<script setup lang="ts">
/**
 * 单个「时刻」下拉（15 分钟档）。
 *
 * 为什么不用 `el-time-select`：它的 `end="24:00"` **选不出 24:00** ——
 * 内部用 `dayjs(current, 'HH:mm').format('HH:mm')` 生成选项，24:00 会被归一成
 * 次日 00:00，于是列表末尾多出一个值为 `00:00` 的重复项（用户以为选的是
 * 「当天结束」，实际拿到的是「当天开始」）。这里自己按档位生成选项，
 * 末档用字面量 `24:00`，「结束时间」才提供它。
 *
 * `step` 只支持 `HH:mm` 形式（如 `00:15`），超出一小时的档位没有使用场景。
 */
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    modelValue?: string
    /** 档位间隔，`HH:mm`，默认 15 分钟（与「录像计划」一致） */
    step?: string
    /** 是否提供末档 `24:00`（= 当天结束 / 次日 00:00）——只有「结束时间」需要 */
    includeDayEnd?: boolean
    placeholder?: string
  }>(),
  {
    modelValue: '',
    step: '00:15',
    includeDayEnd: false,
    placeholder: ''
  }
)

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const value = computed({
  get: () => props.modelValue,
  set: (v: string) => emit('update:modelValue', v)
})

const clocks = computed(() => {
  const parsed = /^(\d{1,2}):(\d{2})$/.exec(props.step)
  const step = parsed ? Number(parsed[1]) * 60 + Number(parsed[2]) : 15
  const pad = (n: number) => String(n).padStart(2, '0')
  const out: string[] = []
  for (let m = 0; m < 24 * 60; m += Math.max(1, step)) {
    out.push(`${pad(Math.floor(m / 60))}:${pad(m % 60)}`)
  }
  return out
})
</script>

<style scoped>
.gb-time-select {
  width: 104px;
}
</style>
