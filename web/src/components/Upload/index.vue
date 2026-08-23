<template>
  <el-upload
    :action="action"
    :headers="headers"
    :data="data"
    :name="name"
    :accept="accept"
    :multiple="multiple"
    :drag="drag"
    :before-upload="beforeUpload"
    :on-success="onSuccess"
    :on-error="onError"
    :show-file-list="showFileList"
    v-model:file-list="fileList"
  >
    <slot>
      <el-button type="primary">
        <el-icon><Upload /></el-icon>
        <span>{{ buttonText }}</span>
      </el-button>
    </slot>
    <template v-if="$slots.tip" #tip>
      <slot name="tip" />
    </template>
  </el-upload>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { ElMessage, type UploadFile, type UploadRawFile, type UploadFiles } from 'element-plus'
import { Upload } from '@element-plus/icons-vue'
import { getToken } from '@/utils/auth'

const props = withDefaults(
  defineProps<{
    action: string
    name?: string
    accept?: string
    multiple?: boolean
    drag?: boolean
    maxSizeMB?: number
    showFileList?: boolean
    buttonText?: string
    data?: Record<string, unknown>
  }>(),
  {
    name: 'file',
    multiple: false,
    drag: false,
    maxSizeMB: 50,
    showFileList: true,
    buttonText: '点击上传'
  }
)

const emit = defineEmits<{
  (e: 'success', res: unknown, file: UploadFile): void
  (e: 'error', err: Error, file: UploadFile): void
  (e: 'change', file: UploadFile, fileList: UploadFiles): void
}>()

const fileList = ref<UploadFiles>([])
const headers = computed(() => ({ 'access-token': getToken() }))

function beforeUpload(file: UploadRawFile): boolean {
  const maxBytes = props.maxSizeMB * 1024 * 1024
  if (file.size > maxBytes) {
    ElMessage.error(`文件超过 ${props.maxSizeMB}MB`)
    return false
  }
  return true
}

function onSuccess(res: unknown, file: UploadFile) {
  ElMessage.success(`${file.name} 上传成功`)
  emit('success', res, file)
}

function onError(_err: Error, file: UploadFile) {
  ElMessage.error(`${file.name} 上传失败`)
  emit('error', _err, file)
}
</script>
