import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'
import Layout from '@/layout/index.vue'

const constantRoutes: RouteRecordRaw[] = [
  {
    path: '/login',
    component: () => import('@/views/login/index.vue'),
    meta: { title: '登录' },
    props: true
  },
  {
    path: '/404',
    component: () => import('@/views/404.vue'),
    meta: { title: '404' }
  },
  {
    path: '/',
    component: Layout,
    redirect: '/dashboard',
    children: [
      {
        path: 'dashboard',
        name: 'Dashboard',
        component: () => import('@/views/dashboard/index.vue'),
        meta: { title: '控制台', icon: 'dashboard', affix: true }
      },
      {
        path: 'device',
        name: 'Device',
        component: () => import('@/views/device/index.vue'),
        meta: { title: '国标设备', icon: 'video-camera' }
      },
      {
        path: 'channel',
        name: 'Channel',
        component: () => import('@/views/channel/index.vue'),
        meta: { title: '通道列表', icon: 'list' }
      },
      {
        path: 'live',
        name: 'Live',
        component: () => import('@/views/live/index.vue'),
        meta: { title: '实时直播', icon: 'video-play' }
      },
      {
        path: 'playback',
        name: 'Playback',
        component: () => import('@/views/playback/index.vue'),
        meta: { title: '录像回放', icon: 'video-camera-filled' }
      },
      {
        path: 'cloudRecord',
        name: 'CloudRecord',
        component: () => import('@/views/cloudRecord/index.vue'),
        meta: { title: '云端录像', icon: 'folder' }
      },
      {
        path: 'mediaServer',
        name: 'MediaServer',
        component: () => import('@/views/mediaServer/index.vue'),
        meta: { title: '媒体节点', icon: 'cpu' }
      },
      {
        path: 'recordPlan',
        name: 'RecordPlan',
        component: () => import('@/views/recordPlan/index.vue'),
        meta: { title: '录像计划', icon: 'calendar' }
      },
      {
        path: 'platform',
        name: 'Platform',
        component: () => import('@/views/platform/index.vue'),
        meta: { title: '上级平台', icon: 'connection' }
      },
      {
        path: 'streamProxy',
        name: 'StreamProxy',
        component: () => import('@/views/streamProxy/index.vue'),
        meta: { title: '拉流代理', icon: 'refresh' }
      },
      {
        path: 'streamPush',
        name: 'StreamPush',
        component: () => import('@/views/streamPush/index.vue'),
        meta: { title: '推流列表', icon: 'upload' }
      },
      {
        path: 'map',
        name: 'Map',
        component: () => import('@/views/map/index.vue'),
        meta: { title: '电子地图', icon: 'map-location' }
      },
      {
        path: 'alarm',
        name: 'Alarm',
        component: () => import('@/views/alarm/index.vue'),
        meta: { title: '报警管理', icon: 'warning' }
      },
      {
        path: 'user',
        name: 'User',
        component: () => import('@/views/user/index.vue'),
        meta: { title: '用户管理', icon: 'user' }
      },
      {
        path: 'jtDevice',
        name: 'JtDevice',
        component: () => import('@/views/jtDevice/index.vue'),
        meta: { title: 'JT1078 终端', icon: 'van' }
      },
      {
        path: 'operations/realLog',
        name: 'RealLog',
        component: () => import('@/views/operations/realLog.vue'),
        meta: { title: '实时日志', icon: 'log' }
      },
      {
        path: 'operations/historyLog',
        name: 'HistoryLog',
        component: () => import('@/views/operations/historyLog.vue'),
        meta: { title: '历史日志', icon: 'historyLog' }
      },
      {
        path: 'operations/systemInfo',
        name: 'SystemInfo',
        component: () => import('@/views/operations/systemInfo.vue'),
        meta: { title: '系统信息', icon: 'systemInfo' }
      }
    ]
  }
]

const router = createRouter({
  history: createWebHashHistory(),
  scrollBehavior: () => ({ left: 0, top: 0 }),
  routes: constantRoutes
})

export function resetRouter() {
  const newRouter = createRouter({
    history: createWebHashHistory(),
    scrollBehavior: () => ({ left: 0, top: 0 }),
    routes: constantRoutes
  })
  ;(router as unknown as { matcher: unknown }).matcher = (newRouter as unknown as { matcher: unknown }).matcher
}

export default router
