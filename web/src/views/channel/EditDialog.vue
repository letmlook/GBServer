<template>
  <el-dialog v-model="visible" :title="isEdit ? '编辑通道' : '新增通道'" width="640px" @open="onOpen">
    <el-form ref="formRef" :model="form" :rules="rules" label-width="100px">
      <el-form-item label="通道国标ID" prop="channelId">
        <el-input v-model="form.channelId" placeholder="20 位国标ID" :disabled="isEdit" />
      </el-form-item>
      <el-form-item label="所属设备" prop="deviceId">
        <el-input v-model="form.deviceId" placeholder="父设备国标ID" :disabled="isEdit" />
      </el-form-item>
      <el-form-item label="通道名称" prop="name">
        <el-input v-model="form.name" placeholder="通道名称" />
      </el-form-item>
      <el-form-item label="行政区划">
        <el-input v-model="form.civilCode" placeholder="6 位行政区划码" />
      </el-form-item>
      <el-form-item label="行业">
        <el-select v-model="form.manufacturer" placeholder="行业类型" clearable style="width: 100%">
          <el-option v-for="x in industryList" :key="x" :label="x" :value="x" />
        </el-select>
      </el-form-item>
      <el-form-item label="网络标识">
        <el-select v-model="form.streamIdentification" placeholder="码流" clearable style="width: 100%">
          <el-option v-for="x in networkList" :key="x" :label="x" :value="x" />
        </el-select>
      </el-form-item>
      <el-form-item label="类型">
        <el-select v-model="form.channelType" placeholder="类型" clearable style="width: 100%">
          <el-option v-for="x in typeList" :key="x" :label="x" :value="x" />
        </el-select>
      </el-form-item>
      <el-form-item label="安装地址">
        <el-input v-model="form.address" type="textarea" :rows="2" placeholder="详细地址" />
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
import { addChannel, updateChannel } from '@/api/channel'

const props = defineProps<{
  modelValue: boolean
  channel?: any
  industryList?: string[]
  typeList?: string[]
  networkList?: string[]
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'saved'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v)
})

const isEdit = computed(() => !!props.channel?.id)
const saving = ref(false)
const formRef = ref<FormInstance>()

const form = reactive({
  id: undefined as number | undefined,
  channelId: '',
  deviceId: '',
  name: '',
  civilCode: '',
  manufacturer: '',
  streamIdentification: '',
  channelType: '',
  address: ''
})

const rules: FormRules = {
  channelId: [{ required: true, message: '请输入通道国标ID', trigger: 'blur' }],
  deviceId: [{ required: true, message: '请输入所属设备ID', trigger: 'blur' }],
  name: [{ required: true, message: '请输入通道名称', trigger: 'blur' }]
}

function onOpen() {
  if (props.channel) {
    Object.assign(form, {
      id: props.channel.id,
      channelId: props.channel.channelId,
      deviceId: props.channel.deviceId,
      name: props.channel.name,
      civilCode: props.channel.civilCode,
      manufacturer: props.channel.manufacturer,
      streamIdentification: props.channel.streamIdentification,
      channelType: props.channel.channelType,
      address: props.channel.address
    })
  } else {
    Object.assign(form, {
      id: undefined,
      channelId: '',
      deviceId: '',
      name: '',
      civilCode: '',
      manufacturer: '',
      streamIdentification: '',
      channelType: '',
      address: ''
    })
  }
}

async function onSave() {
  if (!formRef.value) return
  await formRef.value.validate()
  saving.value = true
  try {
    const payload = { ...form, channelType: form.channelType ? Number(form.channelType) : undefined }
    if (isEdit.value) {
      await updateChannel(payload)
      ElMessage.success('已保存')
    } else {
      await addChannel(payload)
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
