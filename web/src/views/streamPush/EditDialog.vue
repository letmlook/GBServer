<template>
  <el-dialog v-model="visible" :title="isEdit ? '编辑推流' : '新增推流'" width="640px" @open="onOpen">
    <el-form ref="formRef" :model="form" :rules="rules" label-width="100px">
      <el-form-item label="App" prop="app">
        <el-input v-model="form.app" />
      </el-form-item>
      <el-form-item label="Stream" prop="stream">
        <el-input v-model="form.stream" />
      </el-form-item>
      <el-form-item label="媒体节点">
        <el-input v-model="form.mediaServerId" placeholder="auto / 节点 ID" />
      </el-form-item>
      <el-form-item label="拉起离线推流">
        <el-switch v-model="form.startOfflinePush" />
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
import { addStreamPush, updateStreamPush, type StreamPush } from '@/api/streamPush'

const props = defineProps<{
  modelValue: boolean
  push?: Partial<StreamPush>
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'saved'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v)
})

const isEdit = computed(() => !!props.push?.id)
const saving = ref(false)
const formRef = ref<FormInstance>()

// 与 WVP 的推流编辑框一致：只有 App / Stream / 媒体节点 / 拉起离线推流。
// 「源 URL」在 WVP 里并不存在（列表展示的是**推流地址**，由 app/stream/节点算出来）。
const form = reactive<Partial<StreamPush>>({
  app: '',
  stream: '',
  mediaServerId: 'auto',
  startOfflinePush: true
})

const rules: FormRules = {
  app: [{ required: true, message: '请输入 App', trigger: 'blur' }],
  stream: [{ required: true, message: '请输入 Stream', trigger: 'blur' }]
}

function onOpen() {
  if (props.push) {
    Object.assign(form, props.push)
  } else {
    Object.assign(form, { app: '', stream: '', mediaServerId: 'auto', startOfflinePush: true })
  }
}

async function onSave() {
  if (!formRef.value) return
  await formRef.value.validate()
  saving.value = true
  try {
    if (isEdit.value) {
      await updateStreamPush(form)
      ElMessage.success('已保存')
    } else {
      await addStreamPush(form)
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
