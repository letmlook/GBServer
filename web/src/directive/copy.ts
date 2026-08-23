/**
 * 全局指令：复制到剪贴板（Vue 3 版）
 * 用法：v-copy="text"
 */
import type { App, DirectiveBinding } from 'vue'
import { ElMessage } from 'element-plus'

interface CopyHTMLElement extends HTMLElement {
  copyData?: string
}

function handleClick(el: CopyHTMLElement, binding: DirectiveBinding<string | (() => string)>) {
  const value = typeof binding.value === 'function' ? binding.value() : binding.value
  el.copyData = value
  if (!value) {
    ElMessage.warning('复制内容不能为空')
    return
  }
  if (navigator.clipboard) {
    navigator.clipboard.writeText(value).then(
      () => ElMessage.success('复制成功'),
      () => fallback(el, value)
    )
  } else {
    fallback(el, value)
  }
}

function fallback(el: CopyHTMLElement, value: string) {
  const textarea = document.createElement('textarea')
  textarea.value = value
  textarea.style.position = 'fixed'
  textarea.style.opacity = '0'
  document.body.appendChild(textarea)
  textarea.select()
  try {
    document.execCommand('copy')
    ElMessage.success('复制成功')
  } catch {
    ElMessage.error('复制失败')
  } finally {
    document.body.removeChild(textarea)
  }
}

export function setupCopyDirective(app: App) {
  app.directive('copy', {
    mounted(el, binding) {
      (el as CopyHTMLElement).copyData = ''
      el.addEventListener('click', () => handleClick(el as CopyHTMLElement, binding))
    }
  })
}

export default setupCopyDirective
