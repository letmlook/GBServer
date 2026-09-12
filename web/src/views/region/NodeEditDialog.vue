<template>
  <el-dialog v-model="visible" :title="title" width="560px" @open="onOpen">
    <el-form ref="formRef" :model="form" :rules="rules" label-width="120px">
      <el-form-item label="上级节点">
        <el-tree-select
          v-model="form.parentId"
          :data="parentOptions"
          :props="{ label: 'name', children: 'children' }"
          node-key="id"
          check-strictly
          default-expand-all
          clearable
          style="width: 100%"
          :placeholder="isEdit ? '不选表示不改动上级' : '不选表示顶级节点'"
        />
      </el-form-item>
      <el-form-item label="国标编码" prop="deviceId">
        <el-input v-model="form.deviceId" :placeholder="deviceIdPlaceholder" />
      </el-form-item>
      <el-form-item label="名称" prop="name">
        <el-input v-model="form.name" placeholder="节点名称" />
      </el-form-item>
      <el-form-item v-if="isGroup" label="业务分组">
        <el-input v-model="form.businessGroup" placeholder="businessGroup，默认 0" />
      </el-form-item>
      <el-form-item v-if="isGroup" label="行政区划">
        <el-input v-model="form.civilCode" placeholder="如 340200" />
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
import {
  addRegion,
  updateRegion,
  addGroup,
  updateGroup,
  type Region,
  type Group
} from '@/api/region'

const props = defineProps<{
  modelValue: boolean
  /** `region` = 行政区划，`group` = 业务分组 */
  kind: 'region' | 'group'
  /** 被编辑的节点（为空表示新增） */
  node?: Record<string, any> | null
  /** 新增时预选的上级节点 */
  defaultParentId?: number
  /** 上级节点下拉的完整树 */
  tree: Array<Record<string, any>>
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'saved'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v)
})

const isGroup = computed(() => props.kind === 'group')
const isEdit = computed(() => !!props.node?.id)
const title = computed(() =>
  `${isEdit.value ? '编辑' : '新增'}${isGroup.value ? '业务分组' : '行政区划'}`
)
const deviceIdPlaceholder = computed(() =>
  isGroup.value ? '分组国标编码（20 位）' : '行政区划国标编码（20 位）'
)

const saving = ref(false)
const formRef = ref<FormInstance>()

const form = reactive<Partial<Region & Group>>({
  parentId: undefined,
  deviceId: '',
  name: '',
  businessGroup: undefined,
  civilCode: undefined
})

const rules: FormRules = {
  name: [{ required: true, message: '请输入名称', trigger: 'blur' }],
  deviceId: [
    { required: true, message: '请输入国标编码', trigger: 'blur' },
    { pattern: /^\d{20}$/, message: '国标编码必须是 20 位数字', trigger: 'blur' }
  ]
}

// 编辑自身时不能把自己选成上级（会成环）
const parentOptions = computed(() => {
  const strip = (nodes: Array<Record<string, any>>): Array<Record<string, any>> =>
    nodes
      .filter((n) => n.id !== props.node?.id)
      .map((n) => ({ ...n, children: n.children ? strip(n.children) : [] }))
  return strip(props.tree ?? [])
})

function onOpen() {
  if (props.node) {
    Object.assign(form, {
      id: props.node.id,
      parentId: props.node.parentId ?? undefined,
      deviceId: props.node.deviceId ?? '',
      name: props.node.name ?? '',
      businessGroup: props.node.businessGroup,
      civilCode: props.node.civilCode
    })
  } else {
    Object.assign(form, {
      id: undefined,
      // 在某个节点下点"新增子节点"时带上它
      parentId: props.defaultParentId ?? undefined,
      deviceId: '',
      name: '',
      businessGroup: undefined,
      civilCode: undefined
    })
  }
}

async function onSave() {
  if (!formRef.value) return
  await formRef.value.validate()
  saving.value = true
  try {
    // 编辑时 parentId 为 undefined = 不改动（后端 COALESCE）；清空选择 = 移到顶级（-1 哨兵）
    const payload: Record<string, any> = { ...form }
    if (!isEdit.value && payload.parentId === undefined) payload.parentId = -1
    if (isEdit.value && payload.parentId === undefined) delete payload.parentId
    if (isEdit.value) {
      await (isGroup.value ? updateGroup(payload) : updateRegion(payload))
    } else {
      await (isGroup.value ? addGroup(payload) : addRegion(payload))
    }
    ElMessage.success('已保存')
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
