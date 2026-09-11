import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import AutoImport from 'unplugin-auto-import/vite'
import Components from 'unplugin-vue-components/vite'
import { ElementPlusResolver } from 'unplugin-vue-components/resolvers'
import { createSvgIconsPlugin } from 'vite-plugin-svg-icons'
import path from 'node:path'

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '')
  return {
    base: '/',
    publicDir: 'public',
    resolve: {
      alias: {
        '@': path.resolve(__dirname, 'src')
      }
    },
    plugins: [
      vue(),
      AutoImport({
        imports: ['vue', 'vue-router', 'pinia'],
        resolvers: [ElementPlusResolver()],
        dts: 'auto-imports.d.ts',
        eslintrc: { enabled: false }
      }),
      Components({
        resolvers: [ElementPlusResolver()],
        dts: 'components.d.ts',
        dirs: ['src/components']
      }),
      createSvgIconsPlugin({
        iconDirs: [path.resolve(process.cwd(), 'src/icons/svg')],
        symbolId: 'icon-[name]'
      })
    ],
    css: {
      preprocessorOptions: {
        scss: {
          additionalData: `@use "@/styles/_responsive.scss" as *;`
        }
      }
    },
    server: {
      host: '0.0.0.0',
      port: 9528,
      open: true,
      proxy: {
        '/dev-api': {
          target: env.VITE_PROXY_TARGET || 'http://127.0.0.1:18080',
          changeOrigin: true,
          // 修正：这里原先把 `/dev-api` 重写成 **空串**，于是
          // `/dev-api/user/login` → `/user/login`，而后端只注册了
          // `/api/user/login`，未知路径会被 SPA 兜底 `nest_service("/")`
          // 命中，返回 **200 + text/html**（index.html）。前端 axios 拿到
          // 200 却不是 JSON，**开发模式下所有接口都不可用**。
          // 生产用 VITE_APP_BASE_API='/api' 同源直连，因此只有 dev 受影响。
          rewrite: (p) => p.replace(/^\/dev-api/, '/api'),
          // WebSocket 也需要代理：语音对讲音频（/api/talk/audio）与实时推送
          // （/api/ws）都走 WS，未开启时代理会直接握手失败。
          ws: true
        },
        '/static/snap': {
          target: env.VITE_PROXY_TARGET || 'http://127.0.0.1:18080',
          changeOrigin: true
        }
      }
    },
    build: {
      outDir: 'dist',
      assetsDir: 'static',
      sourcemap: false,
      target: 'es2018',
      chunkSizeWarningLimit: 1500,
      rollupOptions: {
        output: {
          manualChunks: {
            'element-plus': ['element-plus', '@element-plus/icons-vue'],
            'vue-vendor': ['vue', 'vue-router', 'pinia']
          }
        }
      }
    }
  }
})
