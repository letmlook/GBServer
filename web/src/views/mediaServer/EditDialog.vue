<template>
  <el-dialog v-model="visible" :title="isEdit ? '编辑媒体节点' : '新增媒体节点'" width="540px" @open="onOpen">
    <el-form ref="formRef" :model="form" :rules="rules" label-width="120px">
      <el-form-item label="节点 ID" prop="id">
        <el-input v-model="form.id" :disabled="isEdit" placeholder="唯一标识" />
      </el-form-item>
      <el-form-item label="IP" prop="ip">
        <el-input v-model="form.ip" />
      </el-form-item>
      <el-form-item label="HTTP 端口" prop="httpPort">
        <el-input-number v-model="form.httpPort" :min="0" :max="65535" />
      </el-form-item>
      <el-form-item label="RTMP 端口">
        <el-input-number v-model="form.rtmpPort" :min="0" :max="65535" />
      </el-form-item>
      <el-form-item label="RTSP 端口">
        <el-input-number v-model="form.rtspPort" :min="0" :max="65535" />
      </el-form-item>
      <el-form-item label="Secret" prop="secret">
        <el-input v-model="form.secret" />
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
import { saveMediaServer, type MediaServer } from '@/api/mediaServer'

const props = defineProps<{
  modelValue: boolean
  server?: MediaServer
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'saved'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v)
})

const isEdit = computed(() => !!props.server?.id)
const saving = ref(false)
const formRef = ref<FormInstance>()

const form = reactive<MediaServer>({
  id: '',
  ip: '',
  httpPort: 80,
  rtmpPort: 1935,
  rtspPort: 554,
  secret: '',
  enabled: true
})

const rules: FormRules = {
  id: [{ required: true, message: '请输入节点 ID', trigger: 'blur' }],
  ip: [{ required: true, message: '请输入 IP', trigger: 'blur' }],
  httpPort: [{ required: true, message: '请输入端口', trigger: 'blur' }],
  secret: [{ required: true, message: '请输入 secret', trigger: 'blur' }]
}

function onOpen() {
  if (props.server) {
    Object.assign(form, props.server)
  } else {
    Object.assign(form, {
      id: '',
      ip: '',
      httpPort: 80,
      rtmpPort: 1935,
      rtspPort: 554,
      secret: '',
      enabled: true
    })
  }
}

async function onSave() {
  if (!formRef.value) return
  await formRef.value.validate()
  saving.value = true
  try {
    await saveMediaServer(form)
    ElMessage.success(isEdit.value ? '已保存' : '新增成功')
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
