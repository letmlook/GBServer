<template>
  <div class="tags-bar" role="tablist" aria-label="已打开页面">
    <span
      v-for="tag in tagsView.visitedViews"
      :key="tag.path"
      class="tag"
      :class="{ 'is-active': isActive(tag.path) }"
      role="tab"
      :aria-selected="isActive(tag.path)"
      @click="openTag(tag.path)"
      @contextmenu.prevent="openMenu(tag, $event)"
    >
      <span>{{ tag.title }}</span>
      <i
        v-if="!isAffix(tag)"
        class="el-icon-close close"
        @click.stop="closeSelectedTag(tag)"
      />
    </span>

    <ul
      v-show="contextVisible"
      :style="{ left: left + 'px', top: top + 'px' }"
      class="contextmenu"
    >
      <li @click="refreshTag">刷新</li>
      <li v-if="selectedTag && !isAffix(selectedTag)" @click="closeSelectedTag(selectedTag)">关闭</li>
      <li @click="closeOthers">关闭其他</li>
      <li @click="closeAll">关闭所有</li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useTagsViewStore, type TagView } from '@/store/modules/tagsView'

const route = useRoute()
const router = useRouter()
const tagsView = useTagsViewStore()

const contextVisible = ref(false)
const left = ref(0)
const top = ref(0)
const selectedTag = ref<TagView | null>(null)

function isActive(p: string): boolean {
  return p === route.path
}
function isAffix(tag: TagView): boolean {
  return Boolean(tag.meta?.affix)
}
function openTag(path: string) {
  router.push(path).catch(() => {
    /* navigation duplicated */
  })
}
function closeSelectedTag(tag: TagView) {
  tagsView.delView(tag)
  if (isActive(tag.path)) {
    const last = tagsView.visitedViews.slice(-1)[0]
    if (last) router.push(last.path)
    else router.push('/')
  }
  hideMenu()
}
function refreshTag() {
  if (!selectedTag.value) return hideMenu()
  tagsView.delCachedView(selectedTag.value)
  router.replace('/redirect' + selectedTag.value.path)
  hideMenu()
}
function closeOthers() {
  if (!selectedTag.value) return hideMenu()
  tagsView.delOthersViews(selectedTag.value)
  router.push(selectedTag.value.path)
  hideMenu()
}
function closeAll() {
  tagsView.delAllViews()
  router.push('/')
  hideMenu()
}
function openMenu(tag: TagView, e: MouseEvent) {
  selectedTag.value = tag
  left.value = e.clientX
  top.value = e.clientY
  contextVisible.value = true
}
function hideMenu() {
  contextVisible.value = false
  selectedTag.value = null
}
onBeforeUnmount(() => {
  document.body.removeEventListener('click', hideMenu)
})
document.body.addEventListener('click', hideMenu)
</script>

<style lang="scss" scoped>
.tags-bar {
  flex: 0 0 38px;
  min-height: 38px;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 16px;
  background: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
  overflow-x: auto;
  white-space: nowrap;
}
.tag {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  font-size: var(--text-xs);
  color: var(--text-secondary);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all 0.15s;

  &:hover { color: var(--brand-primary-500); border-color: var(--brand-primary-300); }
  &.is-active {
    background: rgba(11, 138, 178, 0.10);
    color: var(--brand-primary-600);
    border-color: rgba(11, 138, 178, 0.30);
    font-weight: 600;
  }
  .close { font-size: 10px; opacity: 0.6; }
  .close:hover { opacity: 1; }
}

.contextmenu {
  position: fixed;
  list-style: none;
  margin: 0;
  padding: 4px 0;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  box-shadow: var(--shadow-pop);
  font-size: var(--text-xs);
  color: var(--text-secondary);
  z-index: 100;
  li {
    padding: 6px 14px;
    cursor: pointer;
    &:hover { background: var(--bg-hover); color: var(--brand-primary-500); }
  }
}
</style>
