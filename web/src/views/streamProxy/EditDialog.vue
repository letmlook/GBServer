<template>
  <el-dialog v-model="visible" :title="isEdit ? '编辑代理' : '新增拉流代理'" width="640px" @open="onOpen">
    <el-form ref="formRef" :model="form" :rules="rules" label-width="100px">
      <el-form-item label="名称" prop="name">
        <el-input v-model="form.name" />
      </el-form-item>
      <el-form-item label="类型">
        <el-select v-model="form.type" style="width: 100%">
          <el-option label="rtsp" value="rtsp" />
          <el-option label="rtmp" value="rtmp" />
          <el-option label="hls" value="hls" />
        </el-select>
      </el-form-item>
      <el-form-item label="App">
        <el-input v-model="form.app" placeholder="live / 点播 app 名" />
      </el-form-item>
      <el-form-item label="Stream" prop="stream">
        <el-input v-model="form.stream" placeholder="流 ID" />
      </el-form-item>
      <el-form-item label="源 URL" prop="url">
        <el-input v-model="form.url" placeholder="rtsp://... 或 rtmp://..." />
      </el-form-item>
      <el-form-item label="目标 URL">
        <el-input v-model="form.destUrl" placeholder="(可选) 国标转发目标" />
      </el-form-item>
      <el-form-item label="启用">
        <el-switch v-model="form.enabled" />
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
import { addStreamProxy, updateStreamProxy, type StreamProxy } from '@/api/streamProxy'

const props = defineProps<{
  modelValue: boolean
  proxy?: Partial<StreamProxy>
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'saved'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v)
})

const isEdit = computed(() => !!props.proxy?.id)
const saving = ref(false)
const formRef = ref<FormInstance>()

const form = reactive<Partial<StreamProxy>>({
  name: '',
  type: 'rtsp',
  app: 'live',
  stream: '',
  url: '',
  destUrl: '',
  enabled: true
})

const rules: FormRules = {
  name: [{ required: true, message: '请输入名称', trigger: 'blur' }],
  stream: [{ required: true, message: '请输入流 ID', trigger: 'blur' }],
  url: [{ required: true, message: '请输入源 URL', trigger: 'blur' }]
}

function onOpen() {
  if (props.proxy) {
    Object.assign(form, props.proxy)
  } else {
    Object.assign(form, {
      id: undefined,
      name: '',
      type: 'rtsp',
      app: 'live',
      stream: '',
      url: '',
      destUrl: '',
      enabled: true
    })
  }
}

async function onSave() {
  if (!formRef.value) return
  await formRef.value.validate()
  saving.value = true
  try {
    if (isEdit.value) {
      await updateStreamProxy(form)
      ElMessage.success('已保存')
    } else {
      await addStreamProxy(form)
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
