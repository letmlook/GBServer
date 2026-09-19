import axios, { type AxiosInstance, type AxiosRequestConfig, type AxiosResponse } from 'axios'
import { ElMessage, ElMessageBox } from 'element-plus'
import { getToken } from '@/utils/auth'
import { useUserStore } from '@/store/modules/user'

let showLoginConfirm = false

const service: AxiosInstance = axios.create({
  baseURL: import.meta.env.VITE_APP_BASE_API,
  timeout: 30000
})

service.interceptors.request.use(
  (config) => {
    if (config.url && !config.url.includes('/api/user/login')) {
      const token = getToken()
      if (token) {
        config.headers['access-token'] = token
      }
    }
    return config
  },
  (error) => {
    return Promise.reject(error)
  }
)

service.interceptors.response.use(
  (response: AxiosResponse) => {
    if (response.config.url?.includes('/api/user/logout')) {
      return response.data
    }
    const res = response.data as WvpResult
    if (res && typeof res === 'object' && 'code' in res && res.code !== 0) {
      ElMessage.error({ message: res.msg, showClose: true })
      return Promise.reject(new Error(res.msg || 'Error'))
    }
    return res
  },
  (error) => {
    if (!error.response) {
      // 网络层失败（后端在重启、连接被拒、断网）。
      //
      // 后端重启期间仪表盘每 2s 一次的轮询会连续失败，逐条弹红条会把
      // 屏幕刷满，而且看起来像"登录挂了"。这里做去重节流：同一类错误
      // 6 秒内只提示一次，并明确告知"后端可能正在重启"。
      notifyNetworkOnce(error?.message ?? '网络异常')
      return Promise.reject(error)
    }
    const status = error.response.status
    if (status === 401) {
      const userStore = useUserStore()
      if (!showLoginConfirm && userStore.showConfirmBoxForLoginLose) {
        showLoginConfirm = true
        ElMessageBox.confirm('登录已经到期，是否重新登录', '登录确认', {
          confirmButtonText: '重新登录',
          cancelButtonText: '取消',
          type: 'warning'
        })
          .then(() => {
            userStore.resetToken().then(() => location.reload())
          })
          .catch(() => {
            userStore.closeConfirmBoxForLoginLose()
            ElMessage.warning('登录过期提示已经关闭，请注销后重新登录')
          })
      }
    } else {
      const userStore = useUserStore()
      if (userStore.showConfirmBoxForLoginLose) {
        const data = error.response.data as { msg?: string } | undefined
        ElMessage.error({
          message: data?.msg || error.message,
          showClose: true
        })
      }
    }
    return Promise.reject(error)
  }
)

/** 网络层错误的去重节流窗口（毫秒） */
const NETWORK_TOAST_THROTTLE_MS = 6000
let lastNetworkToastAt = 0
let networkToastOpen = false

function notifyNetworkOnce(message: string) {
  const now = Date.now()
  if (networkToastOpen || now - lastNetworkToastAt < NETWORK_TOAST_THROTTLE_MS) return
  lastNetworkToastAt = now
  networkToastOpen = true
  ElMessage({
    message: `无法连接服务器（${message}）。若后端正在重启，稍候会自动恢复。`,
    type: 'warning',
    showClose: true,
    duration: 4000,
    onClose: () => {
      networkToastOpen = false
    }
  })
}

export interface RequestOptions extends AxiosRequestConfig {
  // 扩展点：loading / silent / retry 等
}

export function request<T = unknown>(config: RequestOptions): Promise<T> {
  return service.request<unknown, T>(config)
}

export default service
