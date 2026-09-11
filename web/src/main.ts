import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import store from './store'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
// 中文语言包：不设置时 Element Plus 用英文默认值，`ElMessageBox` / 分页 /
// 日期选择器 / 表格空数据等处会显示 "OK" / "Cancel" / "No Data" 等英文，
// 与整站中文界面不一致。
import zhCn from 'element-plus/es/locale/lang/zh-cn'
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

app.use(store).use(router).use(ElementPlus, { locale: zhCn }).mount('#app')
