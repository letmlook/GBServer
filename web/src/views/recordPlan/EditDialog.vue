<template>
  <el-dialog v-model="visible" :title="isEdit ? '编辑录像计划' : '新增录像计划'" width="540px" @open="onOpen">
    <el-form ref="formRef" :model="form" :rules="rules" label-width="100px">
      <el-form-item label="名称" prop="name">
        <el-input v-model="form.name" />
      </el-form-item>
      <el-form-item label="类型">
        <el-select v-model="form.planType" style="width: 100%">
          <el-option label="全天" value="all_day" />
          <el-option label="时段" value="time_range" />
        </el-select>
      </el-form-item>
      <el-form-item label="开始时间">
        <el-time-picker v-model="form.startTime" placeholder="HH:mm:ss" />
      </el-form-item>
      <el-form-item label="结束时间">
        <el-time-picker v-model="form.endTime" placeholder="HH:mm:ss" />
      </el-form-item>
      <el-form-item label="启用">
        <el-switch v-model="form.enable" />
      </el-form-item>
      <el-form-item label="星期">
        <el-checkbox-group v-model="weekDays">
          <el-checkbox label="1">周一</el-checkbox>
          <el-checkbox label="2">周二</el-checkbox>
          <el-checkbox label="3">周三</el-checkbox>
          <el-checkbox label="4">周四</el-checkbox>
          <el-checkbox label="5">周五</el-checkbox>
          <el-checkbox label="6">周六</el-checkbox>
          <el-checkbox label="0">周日</el-checkbox>
        </el-checkbox-group>
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" :loading="saving" @click="onSave">保存</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ElMessage, type FormInstance, type FormRules } from 'element-plus'
import { addRecordPlan, updateRecordPlan, type RecordPlan } from '@/api/recordPlan'

const props = defineProps<{
  modelValue: boolean
  plan?: Partial<RecordPlan>
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'saved'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v)
})

const isEdit = computed(() => !!props.plan?.id)
const saving = ref(false)
const formRef = ref<FormInstance>()
const weekDays = ref<string[]>(['1', '2', '3', '4', '5', '6', '0'])

const form = reactive<Partial<RecordPlan>>({
  name: '',
  planType: 'all_day',
  startTime: '00:00:00',
  endTime: '23:59:59',
  enable: true
})

const rules: FormRules = {
  name: [{ required: true, message: '请输入名称', trigger: 'blur' }]
}

watch(weekDays, (v) => {
  form.mon = v.includes('1')
  form.tue = v.includes('2')
  form.wed = v.includes('3')
  form.thu = v.includes('4')
  form.fri = v.includes('5')
  form.sat = v.includes('6')
  form.sun = v.includes('0')
}, { deep: true })

function onOpen() {
  if (props.plan) {
    Object.assign(form, props.plan)
    const days: string[] = []
    if (props.plan.mon) days.push('1')
    if (props.plan.tue) days.push('2')
    if (props.plan.wed) days.push('3')
    if (props.plan.thu) days.push('4')
    if (props.plan.fri) days.push('5')
    if (props.plan.sat) days.push('6')
    if (props.plan.sun) days.push('0')
    weekDays.value = days
  } else {
    Object.assign(form, {
      id: undefined,
      name: '',
      planType: 'all_day',
      startTime: '00:00:00',
      endTime: '23:59:59',
      enable: true
    })
    weekDays.value = ['1', '2', '3', '4', '5', '6', '0']
  }
}

async function onSave() {
  if (!formRef.value) return
  await formRef.value.validate()
  saving.value = true
  try {
    if (isEdit.value) {
      await updateRecordPlan(form)
      ElMessage.success('已保存')
    } else {
      await addRecordPlan(form)
      ElMessage.success('新增成功')
    }
    visible.value = false
    emit('saved')
  } catch (e: any) {
    ElMessage.error(e?.message ?? '保存失败')
  } finally {
    saving.value = false
  }
}

watch(() => props.modelValue, (v) => v && onOpen())
</script>
