import { getToken } from '@/utils/auth'
import type { SystemInfo } from '@/api/log'

/**
 * `system/info` 的并发合并（in-flight coalescing）。
 *
 * 为什么需要：该接口在服务端要真采一次 CPU（60ms）与网络速率（2×100ms）再读磁盘，
 * 本机单次实测约 0.8s；而控制台、侧边栏「存储」、导航栏「平台信息」在挂载时会
 * **同时**各拉一次 —— 浏览器实测第三个响应要 2.3~2.5s，控制台里的 CPU/内存/磁盘/
 * 服务健康面板因此要等约 2.5s 才出数（用户看到的"打开控制台要等 2s"）。
 *
 * 这里只合并"同一时刻的重复请求"：**不落缓存、不返回旧值**，每个调用方拿到的都是
 * 本次的真实响应，只是几个组件共用一次网络往返。
 *
 * 用裸 fetch 而不是 axios：本接口都是后台静默调用，失败时只需"保持上一次的值"
 * （控制台另有健康指示灯按 `apiOk.sys` 变红），不应弹业务码/网络错误 toast ——
 * 这与导航栏原先单独用 fetch 的理由一致（见 Navbar.vue::loadPlatformInfo 注释）。
 * 401 由控制台其余 axios 请求的拦截器统一处理。
 */
let inflight: Promise<SystemInfo | null> | null = null

export function loadSystemInfo(): Promise<SystemInfo | null> {
  if (inflight) return inflight

  const base = (import.meta.env.VITE_APP_BASE_API ?? '') as string
  // `_t` 沿用导航栏原来的做法：绕开浏览器对 GET 的启发式缓存
  const url = `${base}/server/system/info?_t=${Date.now()}`

  inflight = fetch(url, {
    credentials: 'include',
    headers: { 'access-token': getToken() ?? '' }
  })
    .then(async (res) => {
      if (!res.ok) return null
      const body = (await res.json().catch(() => null)) as
        | { code?: number; data?: SystemInfo }
        | null
      return (body?.data as SystemInfo) ?? null
    })
    .catch(() => null)
    .finally(() => {
      inflight = null
    })

  return inflight
}
