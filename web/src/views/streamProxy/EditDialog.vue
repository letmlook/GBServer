<template>
  <el-dialog v-model="visible" :title="isEdit ? '编辑代理' : '新增拉流代理'" width="680px" @open="onOpen">
    <el-form ref="formRef" :model="form" :rules="rules" label-width="140px">
      <el-form-item label="名称" prop="name">
        <el-input v-model="form.name" placeholder="代理名称" />
      </el-form-item>
      <el-form-item label="代理方式" prop="type">
        <el-select v-model="form.type" style="width: 100%">
          <el-option label="默认（ZLM 原生拉流）" value="default" />
          <el-option label="FFmpeg 代理" value="ffmpeg" />
        </el-select>
      </el-form-item>
      <el-form-item label="应用名" prop="app">
        <el-input v-model="form.app" placeholder="live / proxy" />
      </el-form-item>
      <el-form-item label="流 ID" prop="stream">
        <el-input v-model="form.stream" placeholder="流 ID" />
      </el-form-item>
      <el-form-item label="拉流地址" prop="srcUrl">
        <el-input v-model="form.srcUrl" placeholder="rtsp://... / rtmp://... / http://...m3u8" />
      </el-form-item>
      <el-form-item label="超时时间(秒)" prop="timeout">
        <el-input-number v-model="form.timeout" :min="1" :max="3600" controls-position="right" />
      </el-form-item>
      <el-form-item label="节点选择">
        <el-select v-model="form.relatesMediaServerId" placeholder="自动选择" clearable style="width: 100%">
          <el-option label="自动选择" value="" />
          <el-option v-for="ms in mediaServers" :key="ms.id" :label="ms.id ?? ''" :value="ms.id ?? ''" />
        </el-select>
      </el-form-item>
      <el-form-item v-if="form.type === 'ffmpeg'" label="FFmpeg 命令模板">
        <el-select v-model="form.ffmpegCmdKey" placeholder="请选择命令模板" clearable style="width: 100%">
          <el-option v-for="(label, key) in ffmpegCmdList" :key="key" :label="label" :value="key" />
        </el-select>
      </el-form-item>
      <el-form-item label="拉流方式(RTSP)">
        <el-select v-model="form.rtspType" placeholder="默认" clearable style="width: 100%">
          <el-option label="TCP" value="0" />
          <el-option label="UDP" value="1" />
          <el-option label="组播" value="2" />
        </el-select>
      </el-form-item>
      <el-form-item label="无人观看">
        <el-radio-group v-model="noneReader">
          <el-radio :label="0">不做处理</el-radio>
          <el-radio :label="1">停用</el-radio>
        </el-radio-group>
      </el-form-item>
      <el-form-item label="其他选项">
        <el-checkbox v-model="form.enable">启用</el-checkbox>
        <el-checkbox v-model="form.enableAudio">开启音频</el-checkbox>
        <el-checkbox v-model="form.enableMp4">录制 MP4</el-checkbox>
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
import { addStreamProxy, updateStreamProxy, getFfmpegCmdList, type StreamProxy } from '@/api/streamProxy'
import { getMediaServerOnlineList, type MediaServer } from '@/api/mediaServer'

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
const mediaServers = ref<MediaServer[]>([])
const ffmpegCmdList = ref<Record<string, string>>({})

// 字段名与后端 / WVP 的 StreamProxy 完全一致（此前用的 url/enabled/destUrl 后端都不认）
const form = reactive<Partial<StreamProxy>>({
  name: '',
  type: 'default',
  app: 'proxy',
  stream: '',
  srcUrl: '',
  timeout: 30,
  relatesMediaServerId: '',
  ffmpegCmdKey: '',
  rtspType: '',
  enable: true,
  enableAudio: false,
  enableMp4: false,
  enableDisableNoneReader: false
})

// 「无人观看」是 0/1 单选，后端字段是布尔 enableDisableNoneReader
const noneReader = computed({
  get: () => (form.enableDisableNoneReader ? 1 : 0),
  set: (v: number) => {
    form.enableDisableNoneReader = v === 1
  }
})

const rules: FormRules = {
  name: [{ required: true, message: '请输入名称', trigger: 'blur' }],
  app: [{ required: true, message: '请输入应用名', trigger: 'blur' }],
  stream: [{ required: true, message: '请输入流 ID', trigger: 'blur' }],
  srcUrl: [{ required: true, message: '请输入拉流地址', trigger: 'blur' }]
}

async function loadMediaServers() {
  if (mediaServers.value.length) return
  try {
    const res = await getMediaServerOnlineList()
    mediaServers.value = res.data ?? []
  } catch {
    mediaServers.value = []
  }
}

async function loadFfmpegCmds() {
  const id = form.relatesMediaServerId || mediaServers.value[0]?.id || 'auto'
  try {
    const res = await getFfmpegCmdList(id)
    ffmpegCmdList.value = res.data ?? {}
  } catch {
    // 节点不可用时保持空列表，用户仍可手填（但与 ZLM 上不存在的 key 一样会被 ZLM 拒绝）
    ffmpegCmdList.value = {}
  }
}

function onOpen() {
  loadMediaServers()
  if (props.proxy) {
    Object.assign(form, {
      enableAudio: false,
      enableMp4: false,
      enableDisableNoneReader: false,
      timeout: 30,
      ...props.proxy
    })
  } else {
    Object.assign(form, {
      id: undefined,
      name: '',
      type: 'default',
      app: 'proxy',
      stream: '',
      srcUrl: '',
      timeout: 30,
      relatesMediaServerId: '',
      ffmpegCmdKey: '',
      rtspType: '',
      enable: true,
      enableAudio: false,
      enableMp4: false,
      enableDisableNoneReader: false
    })
  }
  if (form.type === 'ffmpeg') loadFfmpegCmds()
}

watch(
  () => form.type,
  (t) => {
    if (t === 'ffmpeg') loadFfmpegCmds()
  }
)

watch(
  () => form.relatesMediaServerId,
  () => {
    if (form.type === 'ffmpeg') loadFfmpegCmds()
  }
)

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
