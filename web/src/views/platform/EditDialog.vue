<template>
  <el-dialog v-model="visible" :title="isEdit ? '编辑平台' : '新增上级平台'" width="720px" @open="onOpen">
    <el-form ref="formRef" :model="form" :rules="rules" label-width="130px">
      <el-form-item label="平台名称" prop="name">
        <el-input v-model="form.name" placeholder="上级平台名称" />
      </el-form-item>
      <el-form-item label="国标ID" prop="serverGBId">
        <el-input v-model="form.serverGBId" :disabled="isEdit" placeholder="20 位国标ID" />
      </el-form-item>
      <el-form-item label="国标域" prop="serverGBDomain">
        <el-input v-model="form.serverGBDomain" placeholder="如 3402000000（10 位域编码）" />
      </el-form-item>
      <el-form-item label="IP" prop="serverIp">
        <el-input v-model="form.serverIp" placeholder="上级平台 SIP IP" />
      </el-form-item>
      <el-form-item label="端口" prop="serverPort">
        <el-input-number v-model="form.serverPort" :min="0" :max="65535" controls-position="right" />
      </el-form-item>
      <el-form-item label="设备国标编号">
        <el-input v-model="form.deviceGBId" placeholder="本平台在该上级侧的编号，可留空" />
      </el-form-item>
      <el-form-item label="设备 IP">
        <el-input v-model="form.deviceIp" placeholder="本平台对外 IP，可留空" />
      </el-form-item>
      <el-form-item label="设备端口">
        <el-input v-model="form.devicePort" placeholder="本平台对外 SIP 端口，可留空" />
      </el-form-item>
      <el-form-item label="用户名">
        <el-input v-model="form.username" placeholder="默认使用设备国标编号" />
      </el-form-item>
      <el-form-item label="密码">
        <el-input v-model="form.password" type="password" show-password placeholder="编辑时留空表示不修改" />
      </el-form-item>
      <el-form-item label="传输">
        <el-select v-model="form.transport" style="width: 100%">
          <el-option label="UDP" value="UDP" />
          <el-option label="TCP" value="TCP" />
        </el-select>
      </el-form-item>
      <el-form-item label="注册周期(秒)">
        <el-input-number v-model="form.expires" :min="0" controls-position="right" />
      </el-form-item>
      <el-form-item label="心跳周期(秒)">
        <el-input-number v-model="form.keepTimeout" :min="0" controls-position="right" />
      </el-form-item>
      <el-form-item label="字符集">
        <el-select v-model="form.characterSet" clearable style="width: 100%">
          <el-option label="GB2312" value="GB2312" />
          <el-option label="UTF-8" value="UTF-8" />
        </el-select>
      </el-form-item>
      <el-form-item label="行政区划">
        <el-input v-model="form.civilCode" placeholder="如 340200" />
      </el-form-item>
      <el-form-item label="其他选项">
        <el-checkbox v-model="form.enable">启用</el-checkbox>
        <el-checkbox v-model="form.ptz">允许云台控制</el-checkbox>
        <el-checkbox v-model="form.rtcp">RTCP 保活</el-checkbox>
        <el-checkbox v-model="form.autoPushChannel">自动推送通道变化</el-checkbox>
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

// 字段名与后端 / WVP 的 Platform.java 一致。此前用的 `serverGbId`(小写 b) /
// `realm` / `registerInterval` / `heartBeatInterval` / `heartBeatCount`
// 后端全都不认（前两个是拼写错，后三个字段根本不存在）。
const form = reactive<Partial<Platform>>({
  name: '',
  serverGBId: '',
  serverGBDomain: '',
  serverIp: '',
  serverPort: 5060,
  deviceGBId: '',
  deviceIp: '',
  devicePort: '',
  username: '',
  password: '',
  transport: 'UDP',
  expires: 3600,
  keepTimeout: 60,
  characterSet: 'GB2312',
  civilCode: '',
  enable: true,
  ptz: false,
  rtcp: false,
  autoPushChannel: true
})

const rules: FormRules = {
  name: [{ required: true, message: '请输入平台名称', trigger: 'blur' }],
  serverGBId: [
    { required: true, message: '请输入国标ID', trigger: 'blur' },
    { pattern: /^\d{20}$/, message: '国标ID 必须是 20 位数字', trigger: 'blur' }
  ],
  serverIp: [{ required: true, message: '请输入IP', trigger: 'blur' }],
  serverPort: [{ required: true, message: '请输入端口', trigger: 'blur' }]
}

/** 库里这两列是 varchar，历史数据可能是字符串；el-input-number 只吃 number */
function num(v: unknown, fallback: number): number {
  const n = Number(v)
  return Number.isFinite(n) ? n : fallback
}

function onOpen() {
  if (props.platform) {
    // 编辑时密码不回显（后端返回的是库里的密文/原值，留空表示不修改）
    Object.assign(form, props.platform, {
      password: '',
      expires: num(props.platform.expires, 3600),
      keepTimeout: num(props.platform.keepTimeout, 60)
    })
  } else {
    Object.assign(form, {
      id: undefined,
      name: '',
      serverGBId: '',
      serverGBDomain: '',
      serverIp: '',
      serverPort: 5060,
      deviceGBId: '',
      deviceIp: '',
      devicePort: '',
      username: '',
      password: '',
      transport: 'UDP',
      expires: 3600,
      keepTimeout: 60,
      characterSet: 'GB2312',
      civilCode: '',
      enable: true,
      ptz: false,
      rtcp: false,
      autoPushChannel: true
    })
  }
}

async function onSave() {
  if (!formRef.value) return
  await formRef.value.validate()
  saving.value = true
  try {
    // 编辑时密码留空 = 不修改：传空串会被后端当成新密码写掉
    const payload: Partial<Platform> = { ...form }
    if (isEdit.value && !payload.password) delete payload.password
    if (isEdit.value) {
      await updatePlatform(payload)
      ElMessage.success('已保存')
    } else {
      await addPlatform(payload)
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
