import 'vue-router'
import type { TagView } from '@/store/modules/tagsView'

declare module 'vue-router' {
  interface RouteMeta {
    title?: string
    icon?: string
    affix?: boolean
    noCache?: boolean
    activeMenu?: string
    hidden?: boolean
    alwaysShow?: boolean
    roles?: string[]
    /** 用于登录确认 */
    requiresAuth?: boolean
  }
}

declare global {
  /**
   * 后端 WVPResult 响应：{ code, msg, data }
   * 全局可用，无需重复 import。
   */
  // eslint-disable-next-line @typescript-eslint/no-empty-object-type
  interface WvpResult<T = unknown> {
    code: number
    msg: string
    data: T
  }
}

export type { TagView }
