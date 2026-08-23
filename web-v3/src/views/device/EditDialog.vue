<template>
  <el-dialog v-model="visible" :title="isEdit ? '编辑设备' : '新增设备'" width="680px" @open="onOpen">
    <el-form ref="formRef" :model="form" :rules="rules" label-width="120px">
      <el-form-item label="国标ID" prop="deviceId">
        <el-input v-model="form.deviceId" placeholder="20 位国标ID" :disabled="isEdit" />
      </el-form-item>
      <el-form-item label="设备名称" prop="name">
        <el-input v-model="form.name" />
      </el-form-item>
      <el-form-item label="厂家">
        <el-input v-model="form.manufacturer" />
      </el-form-item>
      <el-form-item label="型号">
        <el-input v-model="form.model" />
      </el-form-item>
      <el-form-item label="IP" prop="ip">
        <el-input v-model="form.ip" />
      </el-form-item>
      <el-form-item label="端口" prop="port">
        <el-input-number v-model="form.port" :min="0" :max="65535" />
      </el-form-item>
      <el-form-item label="信令传输">
        <el-select v-model="form.transport" style="width: 100%">
          <el-option label="UDP" value="UDP" />
          <el-option label="TCP" value="TCP" />
        </el-select>
      </el-form-item>
      <el-form-item label="流传输模式">
        <el-select v-model="form.streamMode" style="width: 100%">
          <el-option label="UDP" value="UDP" />
          <el-option label="TCP" value="TCP" />
          <el-option label="TCP-ACTIVE" value="TCP-ACTIVE" />
          <el-option label="TCP-PASSIVE" value="TCP-PASSIVE" />
        </el-select>
      </el-form-item>
      <el-form-item label="心跳(秒)">
        <el-input-number v-model="form.heartBeatInterval" :min="0" :max="3600" />
      </el-form-item>
      <el-form-item label="心跳次数">
        <el-input-number v-model="form.heartBeatCount" :min="0" :max="10" />
      </el-form-item>
      <el-form-item label="注册有效期(秒)">
        <el-input-number v-model="form.expires" :min="0" />
      </el-form-item>
      <el-form-item label="密码">
        <el-input v-model="form.password" type="password" show-password />
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
import { add, update } from '@/api/device'

const props = defineProps<{
  modelValue: boolean
  device?: any
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'saved'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v)
})

const isEdit = computed(() => !!props.device?.id)
const saving = ref(false)
const formRef = ref<FormInstance>()

const form = reactive({
  id: undefined as number | undefined,
  deviceId: '',
  name: '',
  manufacturer: '',
  model: '',
  ip: '',
  port: 5060,
  transport: 'UDP',
  streamMode: 'UDP',
  heartBeatInterval: 60,
  heartBeatCount: 3,
  expires: 3600,
  password: ''
})

const rules: FormRules = {
  deviceId: [{ required: true, message: '请输入国标ID', trigger: 'blur' }],
  name: [{ required: true, message: '请输入设备名称', trigger: 'blur' }],
  ip: [{ required: true, message: '请输入IP', trigger: 'blur' }]
}

function onOpen() {
  if (props.device) {
    Object.assign(form, props.device, { password: '' })
  } else {
    Object.assign(form, {
      id: undefined,
      deviceId: '',
      name: '',
      manufacturer: '',
      model: '',
      ip: '',
      port: 5060,
      transport: 'UDP',
      streamMode: 'UDP',
      heartBeatInterval: 60,
      heartBeatCount: 3,
      expires: 3600,
      password: ''
    })
  }
}

async function onSave() {
  if (!formRef.value) return
  await formRef.value.validate()
  saving.value = true
  try {
    if (isEdit.value) {
      await update(form)
      ElMessage.success('已保存')
    } else {
      await add(form)
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

watch(() => props.modelValue, (v) => {
  if (v) onOpen()
})
</script>
