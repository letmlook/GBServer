import { defineStore } from 'pinia'
import defaultSettings from '@/settings'

interface SettingsState {
  showSettings: boolean
  fixedHeader: boolean
  sidebarLogo: boolean
  tagsViews: boolean
}

export const useSettingsStore = defineStore('settings', {
  state: (): SettingsState => ({
    showSettings: defaultSettings.showSettings,
    fixedHeader: defaultSettings.fixedHeader,
    sidebarLogo: defaultSettings.sidebarLogo,
    tagsViews: defaultSettings.tagsViews
  }),
  actions: {
    changeSetting(key: keyof SettingsState, value: boolean) {
      if (key in this.$state) {
        ;(this as unknown as Record<string, unknown>)[key] = value
      }
    }
  }
})
