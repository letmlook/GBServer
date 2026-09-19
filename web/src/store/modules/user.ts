import { defineStore } from 'pinia'
import Cookies from 'js-cookie'
import {
  getToken,
  setToken,
  removeToken,
  getName,
  setName,
  removeName,
  getServerId,
  setServerId,
  removeServerId
} from '@/utils/auth'
import { login, logout, getUserInfo, type LoginPayload, type LoginResult, type UserInfoResult } from '@/api/user'

interface UserState {
  token: string
  name: string
  userId: number
  role: string
  serverId: string
  showConfirmBoxForLoginLose: boolean
}

export const useUserStore = defineStore('user', {
  state: (): UserState => ({
    token: getToken() || '',
    name: getName() || '',
    userId: 0,
    role: '超级管理员',
    serverId: getServerId() || '',
    showConfirmBoxForLoginLose: true
  }),
  actions: {
    async login(userInfo: LoginPayload & { remember?: boolean }) {
      const res = (await login(userInfo)) as unknown as ApiResult<LoginResult>
      const data = res.data
      this.token = data.accessToken
      this.name = data.username
      this.userId = data.id ?? 0
      this.serverId = data.serverId
      this.showConfirmBoxForLoginLose = true
      const cookieOpts = { remember: userInfo.remember !== false }
      setToken(data.accessToken, cookieOpts)
      setName(data.username, cookieOpts)
      setServerId(data.serverId, cookieOpts)
      // 拉一次完整 userInfo 同步 id / role / pushKey
      this.userInfo().catch(() => {})
    },
    async userInfo() {
      try {
        const res = (await getUserInfo()) as unknown as ApiResult<UserInfoResult>
        if (res?.data) {
          this.userId = res.data.id ?? this.userId
          if (res.data.username) this.name = res.data.username
        }
        return res?.data
      } catch {
        return null
      }
    },
    async logout() {
      try {
        await logout()
      } finally {
        this.resetState()
      }
    },
    resetToken() {
      return new Promise<void>((resolve) => {
        removeToken()
        this.resetState()
        resolve()
      })
    },
    closeConfirmBoxForLoginLose() {
      this.showConfirmBoxForLoginLose = false
    },
    resetState() {
      this.token = ''
      this.name = ''
      this.userId = 0
      this.serverId = ''
      this.showConfirmBoxForLoginLose = true
      removeToken()
      removeName()
      removeServerId()
    }
  }
})
