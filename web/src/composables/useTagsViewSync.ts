import { watch } from 'vue'
import { useRoute } from 'vue-router'
import { useTagsViewStore } from '@/store/modules/tagsView'
import type { TagView } from '@/store/modules/tagsView'

/**
 * 将当前路由加入已访问的标签页 store。
 * 建议在 Layout 内 setup() 顶层调用一次。
 */
export function useTagsViewSync() {
  const route = useRoute()
  const tagsView = useTagsViewStore()

  const addIfNeeded = () => {
    if (!route.meta?.title) return
    const view: TagView = {
      path: route.path,
      name: route.name as string | symbol | null,
      title: route.meta.title as string,
      meta: { ...route.meta }
    }
    tagsView.addView(view)
  }

  // 初始化一次
  addIfNeeded()

  watch(
    () => route.path,
    () => addIfNeeded()
  )
}
