import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import store from './store'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import * as ElementPlusIconsVue from '@element-plus/icons-vue'

import '@/icons' // svg 雪碧图
import '@/styles/index.scss'
import '@/permission'

// 启动时根据 cookie 恢复主题（dark / light）
import Cookies from 'js-cookie'
if (Cookies.get('gbserver_theme') === 'dark') {
  document.documentElement.classList.add('dark')
}

const app = createApp(App)

// 全局注册 Element Plus 图标（按需注册已覆盖大多数；这里以防第三方组件用）
for (const [key, comp] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, comp as never)
}

app.use(store).use(router).use(ElementPlus).mount('#app')
