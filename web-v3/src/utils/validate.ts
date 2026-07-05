/**
 * URL 校验
 */
export function isExternal(path: string): boolean {
  return /^(https?:|mailto:|tel:)/.test(path)
}

/**
 * 用户名校验
 */
export function validUsername(str: string): boolean {
  return str.trim().length > 0
}

/**
 * 密码校验
 */
export function validPassword(str: string): boolean {
  return str.length >= 1
}

/**
 * URL 参数序列化
 */
export function param2Obj(url: string): Record<string, string> {
  const search = url.split('?')[1]
  if (!search) return {}
  return JSON.parse(
    '{"' +
      decodeURIComponent(search)
        .replace(/"/g, '\\"')
        .replace(/&/g, '","')
        .replace(/=/g, '":"')
        .replace(/\+/g, ' ') +
      '"}'
  )
}
