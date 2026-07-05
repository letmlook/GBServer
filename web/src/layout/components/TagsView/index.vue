<template>
  <div class="tags-bar" role="tablist" aria-label="已打开页面">
    <span
      v-for="tag in visitedViews"
      :key="tag.path"
      class="tag"
      :class="{ 'is-active': isActive(tag) }"
      role="tab"
      :aria-selected="isActive(tag)"
      @click="openTag(tag)"
      @contextmenu.prevent="openMenu(tag, $event)"
    >
      <span>{{ tag.title }}</span>
      <i v-if="!isAffix(tag)" class="el-icon-close close" @click.stop="closeSelectedTag(tag)" />
    </span>

    <ul v-show="visible" :style="{ left: left + 'px', top: top + 'px' }" class="contextmenu">
      <li @click="refreshSelectedTag">刷新</li>
      <li v-if="!isAffix(selectedTag)" @click="closeSelectedTag(selectedTag)">关闭</li>
      <li @click="closeOthersTags">关闭其他</li>
      <li @click="closeAllTags">关闭所有</li>
    </ul>
  </div>
</template>

<script>
export default {
  name: 'TagsView',
  data() {
    return { visible: false, top: 0, left: 0, selectedTag: {} }
  },
  computed: {
    visitedViews() { return this.$store.state.tagsView.visitedViews }
  },
  watch: {
    $route() { this.moveToCurrent() },
    visible(v) {
      if (v) document.body.addEventListener('click', this.closeMenu)
      else document.body.removeEventListener('click', this.closeMenu)
    }
  },
  methods: {
    isActive(r) { return r.path === this.$route.path },
    isAffix(t) { return t.meta && t.meta.affix },
    openTag(tag) {
      this.$router.push(tag.fullPath)
    },
    moveToCurrent() {
      // 简化：依靠 visitedViews 列表位置即可
    },
    refreshSelectedTag() {
      this.$store.dispatch('tagsView/delCachedView', this.selectedTag).then(() => {
        this.$router.replace('/redirect' + this.selectedTag.fullPath)
      })
      this.closeMenu()
    },
    closeSelectedTag(view) {
      this.$store.dispatch('tagsView/delView', view).then(({ visitedViews }) => {
        if (this.isActive(view)) this.toLast(visitedViews)
      })
      this.closeMenu()
    },
    closeOthersTags() {
      this.$router.push(this.selectedTag)
      this.$store.dispatch('tagsView/delOthersViews', this.selectedTag).then(() => this.moveToCurrent())
      this.closeMenu()
    },
    closeAllTags() {
      this.$store.dispatch('tagsView/delAllViews').then(({ visitedViews }) => {
        if (this.affixIncludes(this.selectedTag)) return
        this.toLast(visitedViews)
      })
      this.closeMenu()
    },
    affixIncludes(t) { return (t.meta && t.meta.affix) || false },
    toLast(visited) {
      const last = visited.slice(-1)[0]
      if (last) this.$router.push(last.fullPath)
      else this.$router.push('/')
    },
    openMenu(tag, e) {
      const offsetLeft = this.$el.getBoundingClientRect().left
      const offsetWidth = this.$el.offsetWidth
      const maxLeft = offsetWidth - 110
      const left = e.clientX - offsetLeft + 10
      this.left = left > maxLeft ? maxLeft : left
      this.top = e.clientY
      this.visible = true
      this.selectedTag = tag
    },
    closeMenu() { this.visible = false }
  }
}
</script>

<style lang="scss" scoped>
.contextmenu {
  position: fixed;
  z-index: 3000;
  list-style: none;
  margin: 0;
  padding: 6px 0;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-pop);
  font-size: var(--text-xs);
  color: var(--text-secondary);
  min-width: 110px;

  li {
    padding: 6px 14px;
    cursor: pointer;
    &:hover { background: var(--bg-hover); color: var(--brand-primary-500); }
  }
}
</style>
