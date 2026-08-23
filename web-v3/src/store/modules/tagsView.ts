import { defineStore } from 'pinia'

export interface TagView {
  path: string
  name?: string | symbol | null
  title: string
  affix?: boolean
  noCache?: boolean
  fullPath?: string
  query?: Record<string, unknown>
  params?: Record<string, unknown>
  hash?: string
  meta?: Record<string, unknown>
}

interface TagsViewState {
  visitedViews: TagView[]
  cachedViews: string[]
}

export const useTagsViewStore = defineStore('tagsView', {
  state: (): TagsViewState => ({
    visitedViews: [],
    cachedViews: []
  }),
  actions: {
    addView(view: TagView) {
      this.addVisitedView(view)
      this.addCachedView(view)
    },
    addVisitedView(view: TagView) {
      if (this.visitedViews.some((v) => v.path === view.path)) return
      this.visitedViews.push({
        ...view,
        title: view.title || 'no-name'
      })
    },
    addCachedView(view: TagView) {
      if (!view.name) return
      const name = String(view.name)
      if (this.cachedViews.includes(name)) return
      if (view.meta && (view.meta as Record<string, unknown>).noCache) return
      this.cachedViews.push(name)
    },
    delView(view: TagView) {
      this.delVisitedView(view)
      this.delCachedView(view)
    },
    delVisitedView(view: TagView) {
      const idx = this.visitedViews.findIndex((v) => v.path === view.path)
      if (idx > -1) this.visitedViews.splice(idx, 1)
    },
    delCachedView(view: TagView) {
      if (!view.name) return
      const idx = this.cachedViews.indexOf(String(view.name))
      if (idx > -1) this.cachedViews.splice(idx, 1)
    },
    delOthersViews(view: TagView) {
      this.delOthersVisitedViews(view)
      this.delOthersCachedViews(view)
    },
    delOthersVisitedViews(view: TagView) {
      this.visitedViews = this.visitedViews.filter(
        (v) => v.meta?.affix || v.path === view.path
      )
    },
    delOthersCachedViews(view: TagView) {
      if (!view.name) {
        this.cachedViews = []
        return
      }
      const idx = this.cachedViews.indexOf(String(view.name))
      this.cachedViews = idx > -1 ? this.cachedViews.slice(idx, idx + 1) : []
    },
    delAllViews() {
      this.delAllVisitedViews()
      this.delAllCachedViews()
    },
    delAllVisitedViews() {
      this.visitedViews = this.visitedViews.filter((v) => v.meta?.affix)
    },
    delAllCachedViews() {
      this.cachedViews = []
    },
    updateVisitedView(view: TagView) {
      const target = this.visitedViews.find((v) => v.path === view.path)
      if (target) Object.assign(target, view)
    }
  }
})
