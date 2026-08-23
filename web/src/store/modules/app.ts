import { defineStore } from 'pinia'
import Cookies from 'js-cookie'

interface SidebarState {
  opened: boolean
  withoutAnimation: boolean
}

interface AppState {
  sidebar: SidebarState
  device: 'desktop' | 'mobile'
}

const SIDEBAR_COOKIE = 'sidebarStatus'

export const useAppStore = defineStore('app', {
  state: (): AppState => ({
    sidebar: {
      opened: Cookies.get(SIDEBAR_COOKIE) ? !!+Cookies.get(SIDEBAR_COOKIE)! : true,
      withoutAnimation: false
    },
    device: 'desktop'
  }),
  actions: {
    toggleSidebar() {
      this.sidebar.opened = !this.sidebar.opened
      this.sidebar.withoutAnimation = false
      Cookies.set(SIDEBAR_COOKIE, this.sidebar.opened ? '1' : '0')
    },
    closeSidebar(withoutAnimation: boolean) {
      Cookies.set(SIDEBAR_COOKIE, '0')
      this.sidebar.opened = false
      this.sidebar.withoutAnimation = withoutAnimation
    },
    toggleDevice(device: 'desktop' | 'mobile') {
      this.device = device
    }
  }
})
