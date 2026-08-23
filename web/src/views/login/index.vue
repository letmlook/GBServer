<template>
  <div class="login-page">
    <section class="login-card">
      <div class="login-card__brand">
        <div class="brand-mark">
          <svg viewBox="0 0 24 24" fill="none">
            <rect x="2.5" y="2.5" width="19" height="19" rx="3" stroke="currentColor" stroke-width="1.5" />
            <circle cx="12" cy="12" r="2.4" fill="currentColor" />
            <line x1="12" y1="1" x2="12" y2="3.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
            <line x1="12" y1="20.5" x2="12" y2="23" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
            <line x1="1" y1="12" x2="3.5" y2="12" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
            <line x1="20.5" y1="12" x2="23" y2="12" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
          </svg>
        </div>
        <h1 class="brand-title">GBServer</h1>
        <p class="brand-sub">GB/T 28181 视频融合平台</p>
        <ul class="brand-points">
          <li>多协议接入 · 50,000+ 通道实时调阅</li>
          <li>云端级联 · 跨地域可视域组网</li>
          <li>智能录像 · 行为检测 / 帧级检索</li>
        </ul>
        <footer class="brand-foot">© 2025 GBServer Team · 视频感知 · 行业互联</footer>
      </div>

      <el-form
        ref="formRef"
        :model="form"
        :rules="rules"
        class="login-card__form"
        size="large"
        @submit.prevent="onSubmit"
      >
        <header class="form-head">
          <h2>欢迎回来</h2>
          <p>使用 SIP 平台的统一凭证登录</p>
        </header>
        <el-form-item prop="username">
          <el-input
            v-model="form.username"
            placeholder="用户名 / SIP 编号"
            :prefix-icon="User"
            autocomplete="username"
          />
        </el-form-item>
        <el-form-item prop="password">
          <el-input
            v-model="form.password"
            type="password"
            placeholder="登录密码"
            :prefix-icon="Lock"
            show-password
            autocomplete="current-password"
            @keyup.enter="onSubmit"
          />
        </el-form-item>
        <div class="form-row">
          <el-checkbox v-model="form.remember">7 天免登录</el-checkbox>
          <a class="link" @click="onForget">忘记密码？</a>
        </div>
        <el-button
          type="primary"
          :loading="loading"
          class="submit"
          @click="onSubmit"
        >
          登 录
        </el-button>
        <div class="hint">
          演示账号：<span class="mono">admin / admin</span>
        </div>
      </el-form>
    </section>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage, ElMessageBox, type FormInstance, type FormRules } from 'element-plus'
import { User, Lock } from '@element-plus/icons-vue'
import { useUserStore } from '@/store/modules/user'
import { validUsername, validPassword } from '@/utils/validate'

const route = useRoute()
const router = useRouter()
const userStore = useUserStore()

const formRef = ref<FormInstance>()
const loading = ref(false)
const form = reactive({ username: 'admin', password: 'admin', remember: true })

const rules: FormRules = {
  username: [{ required: true, validator: (_, v: string, cb) => (validUsername(v) ? cb() : cb(new Error('请输入用户名'))) }],
  password: [{ required: true, validator: (_, v: string, cb) => (validPassword(v) ? cb() : cb(new Error('请输入密码'))) }]
}

async function onSubmit() {
  if (!formRef.value) return
  try {
    await formRef.value.validate()
  } catch {
    return
  }
  loading.value = true
  try {
    await userStore.login({ username: form.username, password: form.password })
    ElMessage.success('登录成功')
    const redirect = (route.query.redirect as string) || '/'
    await router.push(redirect)
  } finally {
    loading.value = false
  }
}

function onForget() {
  ElMessageBox.alert(
    '如忘记密码，请联系系统管理员（admin 账号）通过「用户管理 → 重置」功能重置您的密码。\n\n管理员登录后可进入 用户管理 → 选中账号 → 重置密码。',
    '忘记密码',
    { confirmButtonText: '我知道了' }
  )
}
</script>

<style lang="scss" scoped>
.login-page {
  min-height: 100vh;
  display: grid;
  place-items: center;
  background:
    radial-gradient(circle at 12% 20%, rgba(11, 138, 178, 0.18) 0%, transparent 40%),
    radial-gradient(circle at 90% 80%, rgba(11, 138, 178, 0.10) 0%, transparent 36%),
    var(--bg-base);
}

.login-card {
  display: grid;
  grid-template-columns: 360px 1fr;
  width: 880px;
  max-width: 95vw;
  background: var(--bg-surface);
  border-radius: 16px;
  box-shadow: var(--shadow-overlay);
  overflow: hidden;
  border: 1px solid var(--border-subtle);

  @media (max-width: 768px) {
    grid-template-columns: 1fr;
    .login-card__brand { display: none; }
  }

  &__brand {
    color: #fff;
    padding: 36px 32px;
    background:
      radial-gradient(circle at 30% 30%, rgba(255, 255, 255, 0.10) 0%, transparent 60%),
      linear-gradient(135deg, var(--brand-primary-600) 0%, var(--brand-primary-500) 50%, var(--brand-primary-400) 100%);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .brand-mark {
    width: 44px; height: 44px;
    display: grid; place-items: center;
    background: rgba(255, 255, 255, 0.10);
    border-radius: 10px;
    svg { width: 22px; height: 22px; }
  }
  .brand-title { font-size: 26px; font-weight: 700; margin: 4px 0 0; }
  .brand-sub { font-size: 13px; opacity: .85; margin: 0; }
  .brand-points {
    list-style: none; margin: 12px 0 0; padding: 0;
    display: flex; flex-direction: column; gap: 8px;
    font-size: 12px; opacity: .9;
    li::before { content: '✓'; margin-right: 6px; opacity: .75; }
  }
  .brand-foot { font-size: 11px; opacity: .65; margin-top: auto; }

  &__form {
    padding: 40px 44px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    .form-head h2 { font-size: 20px; font-weight: 700; margin: 0; }
    .form-head p { font-size: 12px; color: var(--text-tertiary); margin: 4px 0 24px; }
  }
  .form-row {
    display: flex; align-items: center; justify-content: space-between;
    font-size: var(--text-xs); margin: -4px 0 8px;
    .link { color: var(--brand-primary-500); cursor: pointer; }
  }
  .submit { width: 100%; height: 40px; font-size: 14px; letter-spacing: 4px; }
  .hint { font-size: var(--text-xs); color: var(--text-tertiary); text-align: center; margin-top: 14px; }
}
</style>
