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
      ElMessage.error({ message: error.message, showClose: true })
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

export interface RequestOptions extends AxiosRequestConfig {
  // 扩展点：loading / silent / retry 等
}

export function request<T = unknown>(config: RequestOptions): Promise<T> {
  return service.request<unknown, T>(config)
}

export default service
