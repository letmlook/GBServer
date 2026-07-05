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
import { login, logout, type LoginPayload, type LoginResult } from '@/api/user'

interface UserState {
  token: string
  name: string
  serverId: string
  showConfirmBoxForLoginLose: boolean
}

export const useUserStore = defineStore('user', {
  state: (): UserState => ({
    token: getToken() || '',
    name: getName() || '',
    serverId: getServerId() || '',
    showConfirmBoxForLoginLose: true
  }),
  actions: {
    async login(userInfo: LoginPayload) {
      const res = (await login(userInfo)) as unknown as WvpResult<LoginResult>
      const data = res.data
      this.token = data.accessToken
      this.name = data.username
      this.serverId = data.serverId
      this.showConfirmBoxForLoginLose = true
      setToken(data.accessToken)
      setName(data.username)
      setServerId(data.serverId)
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
      this.serverId = ''
      this.showConfirmBoxForLoginLose = true
      removeToken()
      removeName()
      removeServerId()
    }
  }
})
