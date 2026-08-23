<template>
  <el-dialog v-model="visible" :title="isEdit ? '编辑终端' : '新增 JT1078 终端'" width="540px" @open="onOpen">
    <el-form ref="formRef" :model="form" :rules="rules" label-width="100px">
      <el-form-item label="手机号" prop="phoneNumber">
        <el-input v-model="form.phoneNumber" :disabled="isEdit" />
      </el-form-item>
      <el-form-item label="车牌号">
        <el-input v-model="form.plateNo" />
      </el-form-item>
      <el-form-item label="车牌颜色">
        <el-select v-model="form.plateColor" style="width: 100%">
          <el-option label="蓝" :value="0" />
          <el-option label="黄" :value="1" />
          <el-option label="黑" :value="2" />
          <el-option label="白" :value="3" />
          <el-option label="绿" :value="4" />
        </el-select>
      </el-form-item>
      <el-form-item label="终端型号">
        <el-input v-model="form.model" />
      </el-form-item>
      <el-form-item label="厂商">
        <el-input v-model="form.makerId" />
      </el-form-item>
      <el-form-item label="省域编码">
        <el-input v-model="form.provinceId" />
      </el-form-item>
      <el-form-item label="市域编码">
        <el-input v-model="form.cityId" />
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
import { addJtTerminal, updateJtTerminal, type JtTerminal } from '@/api/jtDevice'

const props = defineProps<{
  modelValue: boolean
  terminal?: Partial<JtTerminal>
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'saved'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v)
})

const isEdit = computed(() => !!props.terminal?.id)
const saving = ref(false)
const formRef = ref<FormInstance>()

const form = reactive<Partial<JtTerminal>>({
  phoneNumber: '',
  plateNo: '',
  plateColor: 0,
  model: '',
  makerId: '',
  provinceId: '',
  cityId: ''
})

const rules: FormRules = {
  phoneNumber: [{ required: true, message: '请输入手机号', trigger: 'blur' }]
}

function onOpen() {
  if (props.terminal) {
    Object.assign(form, props.terminal)
  } else {
    Object.assign(form, {
      id: undefined,
      phoneNumber: '',
      plateNo: '',
      plateColor: 0,
      model: '',
      makerId: '',
      provinceId: '',
      cityId: ''
    })
  }
}

async function onSave() {
  if (!formRef.value) return
  await formRef.value.validate()
  saving.value = true
  try {
    if (isEdit.value) {
      await updateJtTerminal(form)
      ElMessage.success('已保存')
    } else {
      await addJtTerminal(form)
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
