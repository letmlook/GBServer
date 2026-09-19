import Cookies from 'js-cookie'

const TokenKey = 'gbserver_token'
const NameKey = 'gbserver_username'
const ServerIdKey = 'gbserver_server_id'

/**
 * Cookie 持久化策略：
 * - `remember = true`：写 expires（默认 7 天），关浏览器下次打开仍生效
 * - `remember = false`：**不传 expires**，js-cookie 会写成 session cookie，
 *   关浏览器即失效，符合"勾 7 天免登录"的语义
 *
 * 此前 hardcode `expires = 30`，勾选框事实上是死的。
 */
export interface CookieOpts {
  remember?: boolean
  /** 当 remember=true 时有效，单位天；默认 7 */
  days?: number
}

export function getToken(): string | undefined {
  return Cookies.get(TokenKey)
}

export function setToken(token: string, opts: CookieOpts = {}): void {
  Cookies.set(TokenKey, token, cookieOpts(opts))
}

export function removeToken(): void {
  Cookies.remove(TokenKey)
}

export function getName(): string | undefined {
  return Cookies.get(NameKey)
}

export function setName(name: string, opts: CookieOpts = {}): void {
  Cookies.set(NameKey, name, cookieOpts(opts))
}

export function removeName(): void {
  Cookies.remove(NameKey)
}

export function getServerId(): string | undefined {
  return Cookies.get(ServerIdKey)
}

export function setServerId(serverId: string, opts: CookieOpts = {}): void {
  Cookies.set(ServerIdKey, serverId, cookieOpts(opts))
}

export function removeServerId(): void {
  Cookies.remove(ServerIdKey)
}

function cookieOpts(opts: CookieOpts) {
  if (opts.remember === false) {
    // session cookie：浏览器关闭即失效
    return {}
  }
  return { expires: opts.days ?? 7 }
}
