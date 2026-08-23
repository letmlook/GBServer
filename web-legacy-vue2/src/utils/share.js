/**
 * C6: 分享链接相关工具
 */

/**
 * 将后端返回的 expiresAt（Unix 秒）格式化为可读字符串
 * @param {number} expiresAt - Unix 秒
 * @returns {string}
 */
export function formatExpireTime(expiresAt) {
  if (!expiresAt) {
    return '未知'
  }
  const date = new Date(expiresAt * 1000)
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  const hh = String(date.getHours()).padStart(2, '0')
  const mm = String(date.getMinutes()).padStart(2, '0')
  const ss = String(date.getSeconds()).padStart(2, '0')
  return `${y}-${m}-${d} ${hh}:${mm}:${ss}`
}

/**
 * 从当前 URL 提取 share token
 * @returns {string}
 */
export function extractShareToken() {
  const url = new URL(window.location.href)
  return url.searchParams.get('token') || ''
}