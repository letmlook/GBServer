<template>
  <el-dialog
    :model-value="modelValue"
    :title="isEdit ? '编辑录像计划' : '新增录像计划'"
    width="760px"
    @update:model-value="(v: boolean) => emit('update:modelValue', v)"
    @open="onOpen"
  >
    <el-form ref="formRef" :model="form" label-width="90px" v-loading="loading">
      <el-form-item label="计划名称" prop="name" :rules="[{ required: true, message: '请输入计划名称', trigger: 'blur' }]">
        <el-input v-model="form.name" placeholder="例如：机房全天录像" clearable />
      </el-form-item>

      <el-form-item label="定时截图">
        <el-switch v-model="form.snap" />
        <span class="hint">（该字段与 WVP 保持一致，服务端暂未接线，不影响录像）</span>
      </el-form-item>

      <el-form-item label="录像时段">
        <div class="week-editor">
          <div v-for="day in WEEK_DAY_LABELS" :key="day.value" class="day-row">
            <div class="day-label">{{ day.label }}</div>
            <div class="day-windows">
              <div v-for="(w, idx) in windowsFor(day.value)" :key="idx" class="window-row">
                <el-time-select
                  v-model="w.start"
                  start="00:00"
                  end="24:00"
                  step="00:15"
                  placeholder="开始"
                  style="width: 110px"
                />
                <span class="dash">—</span>
                <el-time-select
                  v-model="w.stop"
                  start="00:00"
                  end="24:00"
                  step="00:15"
                  placeholder="结束"
                  style="width: 110px"
                />
                <el-button link type="danger" @click="removeWindow(day.value, idx)">删除</el-button>
              </div>
              <div v-if="windowsFor(day.value).length === 0" class="no-window">未安排</div>
            </div>
            <div class="day-actions">
              <el-button link type="primary" @click="addWindow(day.value)">添加时段</el-button>
              <el-button link type="primary" @click="setFullDay(day.value)">全天</el-button>
              <el-button link type="primary" @click="clearDay(day.value)">清空</el-button>
            </div>
          </div>
          <div class="quick-actions">
            <el-button size="small" @click="copyToAll('1')">周一复制到全部</el-button>
            <el-button size="small" @click="clearAll">全部清空</el-button>
          </div>
        </div>
      </el-form-item>
    </el-form>

    <template #footer>
      <el-button @click="emit('update:modelValue', false)">取消</el-button>
      <el-button type="primary" :loading="saving" @click="onSave">保存</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ElMessage, type FormInstance } from 'element-plus'
import {
  addRecordPlan,
  getRecordPlanOne,
  updateRecordPlan,
  timeStrToMinutes,
  minutesToTimeStr,
  WEEK_DAY_LABELS,
  type RecordPlan,
  type RecordPlanItem
} from '@/api/recordPlan'

const props = defineProps<{
  modelValue: boolean
  plan?: Partial<RecordPlan> | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'saved'): void
}>()

interface TimeWindow {
  start: string
  stop: string
}

const isEdit = computed(() => !!props.plan?.id)
const saving = ref(false)
const loading = ref(false)
const formRef = ref<FormInstance>()

const form = reactive<{ name: string; snap: boolean }>({ name: '', snap: false })
/** ISO 星期（1..7）→ 该天的时段列表 */
const windows = reactive<Record<number, TimeWindow[]>>({})

function windowsFor(day: number): TimeWindow[] {
  if (!windows[day]) windows[day] = []
  return windows[day]
}

function addWindow(day: number) {
  windowsFor(day).push({ start: '00:00', stop: '24:00' })
}

function removeWindow(day: number, idx: number) {
  windowsFor(day).splice(idx, 1)
}

function setFullDay(day: number) {
  windows[day] = [{ start: '00:00', stop: '24:00' }]
}

function clearDay(day: number) {
  windows[day] = []
}

function clearAll() {
  for (const d of WEEK_DAY_LABELS) windows[d.value] = []
}

function copyToAll(fromDay: string) {
  const src = windowsFor(Number(fromDay)).map((w) => ({ ...w }))
  if (src.length === 0) {
    ElMessage.warning('周一没有可复制的时段')
    return
  }
  for (const d of WEEK_DAY_LABELS) {
    if (d.value !== Number(fromDay)) windows[d.value] = src.map((w) => ({ ...w }))
  }
}

function resetForm() {
  form.name = ''
  form.snap = false
  for (const d of WEEK_DAY_LABELS) windows[d.value] = []
}

async function onOpen() {
  resetForm()
  const id = props.plan?.id
  if (!id) {
    // 新增：默认周一~周日全天，符合"开箱即用"的直觉
    for (const d of WEEK_DAY_LABELS) setFullDay(d.value)
    return
  }
  form.name = props.plan?.name ?? ''
  form.snap = !!props.plan?.snap
  loading.value = true
  try {
    // 列表接口不带 planItemList 时才需要再查一次；带上了就直接用
    const plan = props.plan?.planItemList
      ? (props.plan as RecordPlan)
      : (await getRecordPlanOne(id)).data
    for (const item of plan?.planItemList ?? []) {
      if (!item.weekDay || item.start === undefined || item.stop === undefined) continue
      windowsFor(item.weekDay).push({
        start: minutesToTimeStr(item.start),
        stop: minutesToTimeStr(item.stop)
      })
    }
  } catch (e: any) {
    ElMessage.error(e?.message ?? '加载计划失败')
  } finally {
    loading.value = false
  }
}

/** 收集所有时段并做本地校验（后端也会校验，这里是为了给出即时反馈） */
function collectItems(): RecordPlanItem[] | null {
  const items: RecordPlanItem[] = []
  for (const day of WEEK_DAY_LABELS) {
    for (const w of windowsFor(day.value)) {
      if (!w.start || !w.stop) {
        ElMessage.error(`${day.label} 存在未填写完整的时段`)
        return null
      }
      const start = timeStrToMinutes(w.start)
      const stop = timeStrToMinutes(w.stop)
      // 后端是**闭区间** [start, stop]，所以 start === stop 表示"只录那一分钟"，
      // 是合法但几乎肯定不是本意的输入；start > stop 则永远不会触发。
      if (start > stop) {
        ElMessage.error(`${day.label} ${w.start}-${w.stop}：开始时间晚于结束时间（不支持跨天，请拆成两条时段）`)
        return null
      }
      if (start === stop) {
        ElMessage.error(`${day.label} ${w.start}-${w.stop}：时段长度必须大于 0 分钟`)
        return null
      }
      items.push({ start, stop, weekDay: day.value })
    }
  }
  if (items.length === 0) {
    ElMessage.error('至少需要安排一个录像时段')
    return null
  }
  // 同一天内不允许重叠，避免出现"看起来两条、实际等效"的困惑
  for (const day of WEEK_DAY_LABELS) {
    const list = items.filter((i) => i.weekDay === day.value).sort((a, b) => a.start - b.start)
    for (let i = 1; i < list.length; i++) {
      if (list[i].start < list[i - 1].stop) {
        ElMessage.error(
          `${day.label} 的时段 ${minutesToTimeStr(list[i - 1].start)}-${minutesToTimeStr(
            list[i - 1].stop
          )} 与 ${minutesToTimeStr(list[i].start)}-${minutesToTimeStr(list[i].stop)} 重叠`
        )
        return null
      }
    }
  }
  return items
}

async function onSave() {
  if (!formRef.value) return
  await formRef.value.validate().catch(() => Promise.reject())
  if (!form.name.trim()) {
    ElMessage.error('请输入计划名称')
    return
  }
  const planItemList = collectItems()
  if (!planItemList) return

  saving.value = true
  try {
    if (isEdit.value && props.plan?.id) {
      await updateRecordPlan({
        id: props.plan.id,
        name: form.name.trim(),
        snap: form.snap,
        planItemList
      })
      ElMessage.success('已保存')
    } else {
      await addRecordPlan({ name: form.name.trim(), snap: form.snap, planItemList })
      ElMessage.success('新增成功')
    }
    emit('update:modelValue', false)
    emit('saved')
  } catch (e: any) {
    ElMessage.error(e?.message ?? '保存失败')
  } finally {
    saving.value = false
  }
}

watch(
  () => props.modelValue,
  (v) => {
    if (v) onOpen()
  }
)
</script>

<style scoped>
.week-editor {
  width: 100%;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 4px;
  padding: 8px;
}
.day-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 4px 0;
  border-bottom: 1px dashed var(--el-border-color-lighter);
}
.day-row:last-of-type {
  border-bottom: none;
}
.day-label {
  width: 48px;
  flex: none;
  line-height: 32px;
  color: var(--el-text-color-regular);
}
.day-windows {
  flex: 1;
  min-width: 0;
}
.window-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 4px;
}
.dash {
  color: var(--el-text-color-secondary);
}
.no-window {
  line-height: 32px;
  color: var(--el-text-color-placeholder);
  font-size: 12px;
}
.day-actions {
  flex: none;
  display: flex;
  align-items: center;
}
.quick-actions {
  margin-top: 8px;
  display: flex;
  gap: 8px;
}
.hint {
  margin-left: 8px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
</style>
