<template>
  <el-dialog v-model="visible" :title="isEdit ? '编辑平台' : '新增上级平台'" width="640px" @open="onOpen">
    <el-form ref="formRef" :model="form" :rules="rules" label-width="120px">
      <el-form-item label="平台名称" prop="name">
        <el-input v-model="form.name" />
      </el-form-item>
      <el-form-item label="国标ID" prop="serverGbId">
        <el-input v-model="form.serverGbId" :disabled="isEdit" placeholder="20 位国标ID" />
      </el-form-item>
      <el-form-item label="IP" prop="serverIp">
        <el-input v-model="form.serverIp" />
      </el-form-item>
      <el-form-item label="端口" prop="serverPort">
        <el-input-number v-model="form.serverPort" :min="0" :max="65535" />
      </el-form-item>
      <el-form-item label="域名" prop="realm">
        <el-input v-model="form.realm" placeholder="国标域编码" />
      </el-form-item>
      <el-form-item label="用户名">
        <el-input v-model="form.username" />
      </el-form-item>
      <el-form-item label="密码">
        <el-input v-model="form.password" type="password" show-password />
      </el-form-item>
      <el-form-item label="传输">
        <el-select v-model="form.transport" style="width: 100%">
          <el-option label="UDP" value="UDP" />
          <el-option label="TCP" value="TCP" />
        </el-select>
      </el-form-item>
      <el-form-item label="注册间隔(秒)">
        <el-input-number v-model="form.registerInterval" :min="0" />
      </el-form-item>
      <el-form-item label="心跳间隔(秒)">
        <el-input-number v-model="form.heartBeatInterval" :min="0" />
      </el-form-item>
      <el-form-item label="心跳次数">
        <el-input-number v-model="form.heartBeatCount" :min="0" :max="10" />
      </el-form-item>
      <el-form-item label="有效期(秒)">
        <el-input-number v-model="form.expires" :min="0" />
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
import { addPlatform, updatePlatform, type Platform } from '@/api/platform'

const props = defineProps<{
  modelValue: boolean
  platform?: Partial<Platform>
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'saved'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v)
})

const isEdit = computed(() => !!props.platform?.id)
const saving = ref(false)
const formRef = ref<FormInstance>()

const form = reactive<Partial<Platform>>({
  name: '',
  serverGbId: '',
  serverIp: '',
  serverPort: 5060,
  realm: '',
  username: '',
  password: '',
  transport: 'UDP',
  registerInterval: 60,
  heartBeatInterval: 60,
  heartBeatCount: 3,
  expires: 3600
})

const rules: FormRules = {
  name: [{ required: true, message: '请输入平台名称', trigger: 'blur' }],
  serverGbId: [{ required: true, message: '请输入国标ID', trigger: 'blur' }],
  serverIp: [{ required: true, message: '请输入IP', trigger: 'blur' }],
  serverPort: [{ required: true, message: '请输入端口', trigger: 'blur' }]
}

function onOpen() {
  if (props.platform) {
    Object.assign(form, props.platform, { password: '' })
  } else {
    Object.assign(form, {
      id: undefined,
      name: '',
      serverGbId: '',
      serverIp: '',
      serverPort: 5060,
      realm: '',
      username: '',
      password: '',
      transport: 'UDP',
      registerInterval: 60,
      heartBeatInterval: 60,
      heartBeatCount: 3,
      expires: 3600
    })
  }
}

async function onSave() {
  if (!formRef.value) return
  await formRef.value.validate()
  saving.value = true
  try {
    if (isEdit.value) {
      await updatePlatform(form)
      ElMessage.success('已保存')
    } else {
      await addPlatform(form)
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
