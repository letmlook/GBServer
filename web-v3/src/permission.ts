import router from '@/router'
import NProgress from 'nprogress'
import 'nprogress/nprogress.css'
import { getToken, getName, getServerId } from '@/utils/auth'
import { useUserStore } from '@/store/modules/user'
import getPageTitle from '@/utils/get-page-title'

NProgress.configure({ showSpinner: false })

const whiteList = ['/login']

router.beforeEach(async (to, _from, next) => {
  NProgress.start()
  document.title = getPageTitle(to.meta.title)

  const hasToken = getToken()
  const userStore = useUserStore()

  if (hasToken) {
    if (to.path === '/login') {
      next({ path: '/' })
      NProgress.done()
    } else {
      if (!userStore.name) {
        userStore.name = getName() || ''
        userStore.serverId = getServerId() || ''
      }
      next()
    }
  } else if (whiteList.includes(to.path)) {
    next()
  } else {
    next(`/login?redirect=${to.path}`)
    NProgress.done()
  }
})

router.afterEach(() => {
  NProgress.done()
})
