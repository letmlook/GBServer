<template>
  <div class="login-shell">
    <!-- 背景图 -->
    <div class="login-bg" />
    <!-- 左侧蒙层 -->
    <div class="login-mask" />

    <main class="login-main">
      <!-- 左侧品牌区 -->
      <section class="login-brand">
        <div class="brand-top">
          <div class="brand-mark">GB</div>
          <div class="brand-meta">
            <span class="brand-name">GBServer</span>
            <span class="brand-version text-primary-accent">v2.7.4</span>
          </div>
        </div>
        <div class="brand-mid">
          <h1>GB/T 28181<br>视频物联平台</h1>
          <p>让每一路视频信号都精准抵达指挥中枢。</p>
        </div>
        <div class="brand-foot">© 2026 GBServer · All signals live</div>
      </section>

      <!-- 右侧登录控件 -->
      <section class="login-pane">
        <div class="login-card">
          <h2>登录控制台</h2>
          <el-form ref="form" :model="form" :rules="rules" @submit.native.prevent="onSubmit" @keyup.enter.native="onSubmit">
            <div class="field">
              <svg viewBox="0 0 24 24" fill="none" class="field-icon">
                <circle cx="12" cy="8" r="4" stroke="currentColor" stroke-width="1.5" />
                <path d="M4 21c0-4.4 3.6-8 8-8s8 3.6 8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
              </svg>
              <input
                ref="username"
                v-model="form.username"
                type="text"
                placeholder="用户名"
                autocomplete="username"
              >
            </div>
            <div class="field">
              <svg viewBox="0 0 24 24" fill="none" class="field-icon">
                <rect x="4" y="11" width="16" height="10" rx="2" stroke="currentColor" stroke-width="1.5" />
                <path d="M8 11V7a4 4 0 0 1 8 0v4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
              </svg>
              <input
                v-model="form.password"
                :type="showPwd ? 'text' : 'password'"
                placeholder="密码"
                autocomplete="current-password"
              >
              <button type="button" class="field-toggle" :aria-label="showPwd ? '隐藏密码' : '显示密码'" @click="showPwd = !showPwd">
                <svg v-if="!showPwd" viewBox="0 0 24 24" fill="none">
                  <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7S2 12 2 12z" stroke="currentColor" stroke-width="1.5" />
                  <circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.5" />
                </svg>
                <svg v-else viewBox="0 0 24 24" fill="none">
                  <path d="M3 3l18 18" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
                  <path d="M10.5 6.2A9.7 9.7 0 0 1 12 6c7 0 10 6 10 6a17.5 17.5 0 0 1-3 3.7M6.6 6.6C3.7 8.5 2 12 2 12s3 6 10 6c1.7 0 3.2-.3 4.5-.8" stroke="currentColor" stroke-width="1.5" />
                  <path d="M9.9 10a3 3 0 0 0 4.2 4.2" stroke="currentColor" stroke-width="1.5" />
                </svg>
              </button>
            </div>
            <div class="login-options">
              <el-checkbox v-model="form.remember">7 天免登录</el-checkbox>
              <a class="text-primary-accent" href="javascript:;">忘记密码？</a>
            </div>
            <button class="login-submit" :disabled="loading" type="submit">
              <span v-if="!loading">登录</span>
              <span v-else>登录中…</span>
            </button>
            <div class="login-hint">
              <span class="text-tertiary">演示账号：admin / admin</span>
            </div>
          </el-form>
        </div>
      </section>
    </main>
  </div>
</template>

<script>
import { validUsername } from '@/utils/validate'

export default {
  name: 'Login',
  data() {
    return {
      form: { username: 'admin', password: 'admin', remember: false },
      showPwd: false,
      loading: false,
      redirect: undefined,
      rules: {
        username: [{ required: true, trigger: 'blur', validator: (r, v, cb) => validUsername(v) ? cb() : cb(new Error('请输入用户名')) }],
        password: [{ required: true, trigger: 'blur', validator: (r, v, cb) => v ? cb() : cb(new Error('请输入密码')) }]
      }
    }
  },
  watch: {
    $route: {
      handler(route) { this.redirect = route.query && route.query.redirect },
      immediate: true
    }
  },
  mounted() {
    this.$nextTick(() => this.$refs.username && this.$refs.username.focus())
  },
  methods: {
    onSubmit() {
      this.$refs.form.validate(valid => {
        if (!valid) return
        this.loading = true
        this.$store.dispatch('user/login', this.form)
          .then(() => { this.$router.push({ path: this.redirect || '/' }) })
          .catch(err => this.$message && this.$message.error(typeof err === 'string' ? err : (err && err.message) || '登录失败'))
          .finally(() => { this.loading = false })
      })
    }
  }
}
</script>

<style lang="scss" scoped>
.login-shell {
  position: relative;
  min-height: 100vh;
  background: #02080f;
  color: #e2eef9;
  font-family: var(--font-sans);
  overflow: hidden;
}
.login-bg {
  position: fixed; inset: 0;
  background-image: url('/static/images/bg19.webp');
  background-size: cover;
  background-position: center;
  filter: brightness(0.5) contrast(1.05) saturate(0.9);
}
.login-mask {
  position: fixed; inset: 0;
  background: linear-gradient(90deg, rgba(2,8,15,0.88) 0%, rgba(2,8,15,0.65) 40%, rgba(2,8,15,0.15) 100%);
}
.login-main {
  position: relative;
  min-height: 100vh;
  display: flex;
}

/* 左侧品牌区 3 占比 */
.login-brand {
  flex: 3;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 56px;

  .brand-top { display: flex; align-items: center; gap: 14px; }
  .brand-mark {
    width: 44px; height: 44px;
    border-radius: 10px;
    background: var(--brand-primary-500);
    color: #fff; font-weight: 700; font-size: 18px;
    display: grid; place-items: center;
    letter-spacing: -0.5px;
  }
  .brand-meta { display: flex; align-items: baseline; gap: 10px; }
  .brand-name { color: #fff; font-size: 22px; font-weight: 600; letter-spacing: -0.3px; }
  .brand-version { font-size: 12px; color: var(--brand-primary-300); }

  .brand-mid {
    max-width: 560px;
    h1 {
      color: #fff;
      font-weight: 600;
      line-height: 1.05;
      letter-spacing: -0.02em;
      font-size: clamp(40px, 5vw, 64px);
      margin: 0;
    }
    p {
      margin-top: 24px;
      color: rgba(226, 238, 249, 0.7);
      font-size: 15px;
      line-height: 1.6;
      max-width: 460px;
    }
  }
  .brand-foot { font-size: 11px; color: rgba(226, 238, 249, 0.4); }
}

/* 右侧登录区 2 占比 */
.login-pane {
  flex: 2;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 56px;
}
.login-card {
  width: 100%;
  max-width: 360px;

  h2 {
    color: #fff;
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.3px;
    margin: 0 0 36px;
  }
}

.field {
  position: relative;
  margin-bottom: 12px;
  .field-icon {
    position: absolute; left: 14px; top: 50%;
    transform: translateY(-50%);
    width: 16px; height: 16px;
    color: var(--text-tertiary);
    pointer-events: none;
  }
  input {
    width: 100%;
    background: rgba(10, 19, 32, 0.7);
    border: 1px solid #1a2a3d;
    color: #e2eef9;
    font-size: 14px;
    padding: 12px 14px 12px 40px;
    border-radius: 8px;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
    font-family: var(--font-sans);
    &:focus {
      border-color: var(--brand-primary-500);
      box-shadow: 0 0 0 3px rgba(11, 138, 178, 0.2);
    }
    &::placeholder { color: var(--text-tertiary); }
  }
  .field-toggle {
    position: absolute; right: 12px; top: 50%;
    transform: translateY(-50%);
    background: none; border: 0;
    color: var(--text-tertiary);
    width: 18px; height: 18px;
    padding: 0;
    cursor: pointer;
    &:hover { color: var(--brand-primary-300); }
    svg { width: 16px; height: 16px; }
  }
}

.login-options {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin: 14px 0 18px;
  font-size: 12px;
  ::v-deep .el-checkbox { color: rgba(226, 238, 249, 0.7); }
  ::v-deep .el-checkbox__label { font-size: 12px; }
  a { cursor: pointer; }
}
.login-submit {
  width: 100%;
  background: var(--brand-primary-500);
  border: 0;
  color: #fff;
  font-size: 14px;
  letter-spacing: 0.1em;
  padding: 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.15s, transform 0.05s;
  box-shadow: 0 8px 20px rgba(11, 138, 178, 0.3);
  &:hover { background: var(--brand-primary-600); }
  &:active { transform: scale(0.99); }
  &:disabled { opacity: 0.6; cursor: not-allowed; }
}
.login-hint { margin-top: 18px; text-align: center; font-size: 11px; }

@media (max-width: 960px) {
  .login-main { flex-direction: column; }
  .login-brand { display: none; }
  .login-pane { padding: 32px 24px; }
  .login-card { max-width: 100%; }
}
</style>
