<template>
  <el-dialog v-model="visible" title="编辑用户" width="480px" @open="onOpen">
    <el-form ref="formRef" :model="form" :rules="rules" label-width="100px">
      <el-form-item label="用户名" prop="username">
        <el-input v-model="form.username" />
      </el-form-item>
      <el-form-item label="角色" prop="roleId">
        <el-select v-model="form.roleId" style="width: 100%">
          <el-option v-for="r in roles" :key="r.id" :label="r.name" :value="r.id" />
        </el-select>
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
import { updateUser } from '@/api/user'

const props = defineProps<{
  modelValue: boolean
  /** 待编辑的行（含 id / username / role.id） */
  row?: { id?: number; username?: string; role?: { id?: number } } | null
  roles?: { id: number; name: string }[]
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'saved'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v)
})

const saving = ref(false)
const formRef = ref<FormInstance>()

const form = reactive({
  username: '',
  roleId: 0 as number
})

const rules: FormRules = {
  username: [
    { required: true, message: '请输入用户名', trigger: 'blur' },
    { max: 64, message: '用户名最多 64 个字符', trigger: 'blur' }
  ],
  roleId: [{ required: true, message: '请选择角色', trigger: 'change' }]
}

function onOpen() {
  Object.assign(form, {
    username: props.row?.username ?? '',
    roleId: props.row?.role?.id ?? 0
  })
}

async function onSave() {
  if (!formRef.value) return
  await formRef.value.validate()
  const userId = props.row?.id
  if (!userId) {
    ElMessage.error('缺少用户 ID')
    return
  }
  saving.value = true
  try {
    // 只提交真正改动的字段：后端只更新传入项，两者都不传会被拒。
    await updateUser({
      userId,
      username: form.username.trim(),
      roleId: form.roleId
    })
    ElMessage.success('保存成功')
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
