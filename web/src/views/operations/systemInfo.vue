<template>
  <div class="gb-page">
    <div class="gb-page__header">
      <div>
        <h1 class="gb-page__title">平台信息</h1>
        <p class="gb-page__subtitle">GBServer · v2.7.4 · 部署环境与运行时</p>
      </div>
      <div class="gb-page__actions">
        <button class="gb-btn">健康检查</button>
        <button class="gb-btn">配置</button>
        <button class="gb-btn gb-btn--primary">重启服务</button>
      </div>
    </div>

    <section class="gb-grid gb-grid--2col">
      <article class="gb-card">
        <header class="gb-card-title"><span>基本信息</span></header>
        <table class="kv">
          <tr><td class="k">服务名</td><td class="v mono">gbserver</td></tr>
          <tr><td class="k">版本</td><td class="v">v2.7.4 <span class="text-tertiary">(2026-06-30)</span></td></tr>
          <tr><td class="k">构建</td><td class="v mono">#38f1a7c · release · Linux x86_64</td></tr>
          <tr><td class="k">协议</td><td class="v">GB/T 28181-2016 · JT/T 808 · JT/T 1078</td></tr>
          <tr><td class="k">SIP 域</td><td class="v mono">4401000000</td></tr>
          <tr><td class="k">SIP 端口</td><td class="v mono">5060 / 5061 (TLS)</td></tr>
          <tr><td class="k">媒体协议</td><td class="v">RTSP · RTMP · HLS · WebRTC · GB28181</td></tr>
          <tr><td class="k">数据源</td><td class="v">PostgreSQL 15.4</td></tr>
        </table>
      </article>

      <article class="gb-card">
        <header class="gb-card-title"><span>运行时资源</span><span class="meta">采样于 30s 前</span></header>
        <ul class="runtime">
          <li>
            <div class="runtime__row"><span>CPU</span><span class="mono">42%</span></div>
            <div class="gb-progress"><div class="gb-progress__fill" style="width:42%"/></div>
          </li>
          <li>
            <div class="runtime__row"><span>内存</span><span class="mono">5.6 / 16 GB</span></div>
            <div class="gb-progress"><div class="gb-progress__fill gb-progress__fill--warning" style="width:35%"/></div>
          </li>
          <li>
            <div class="runtime__row"><span>磁盘</span><span class="mono">187 / 240 GB</span></div>
            <div class="gb-progress"><div class="gb-progress__fill gb-progress__fill--gradient-warm" style="width:78%"/></div>
          </li>
          <li>
            <div class="runtime__row"><span>网络收发</span><span class="mono">2.4 Gbps</span></div>
            <div class="gb-progress"><div class="gb-progress__fill" style="width:24%"/></div>
          </li>
          <li>
            <div class="runtime__row"><span>连接数</span><span class="mono">4,128</span></div>
            <div class="gb-progress"><div class="gb-progress__fill" style="width:51%"/></div>
          </li>
          <li>
            <div class="runtime__row"><span>运行时长</span><span class="mono">38 天 6 时 14 分</span></div>
            <div class="gb-progress"><div class="gb-progress__fill gb-progress__fill--success" style="width:88%"/></div>
          </li>
        </ul>
      </article>
    </section>

    <section class="gb-grid gb-grid--2col">
      <article class="gb-card">
        <header class="gb-card-title"><span>功能模块</span></header>
        <div class="module-grid">
          <div v-for="m in modules" :key="m.name" class="module">
            <div class="module__name">{{ m.name }}<span class="text-tertiary"> v{{ m.version }}</span></div>
            <div class="module__meta text-tertiary text-xs">{{ m.desc }}</div>
            <span :class="['gb-chip', 'gb-chip--' + (m.enabled ? 'success' : 'mute')]">{{ m.enabled ? '启用' : '未启用' }}</span>
          </div>
        </div>
      </article>

      <article class="gb-card">
        <header class="gb-card-title"><span>近期活动</span></header>
        <ul class="activity">
          <li v-for="a in activity" :key="a.t">
            <span class="mono text-tertiary" style="font-size:11px">{{ a.t }}</span>
            <span style="margin-left:8px">{{ a.text }}</span>
            <span class="text-tertiary text-xs" style="margin-left:6px">· {{ a.by }}</span>
          </li>
        </ul>
      </article>
    </section>
  </div>
</template>

<script>
export default {
  name: 'SystemInfo',
  data() {
    return {
      modules: [
        { name: 'SIP 服务', version: '2.7.4', desc: 'GB28181 注册 / 目录 / 邀请', enabled: true },
        { name: '级联注册', version: '2.7.4', desc: '向上级平台注册和心跳', enabled: true },
        { name: 'JT/T 808', version: '2.7.4', desc: '部标 808 TCP 接入', enabled: true },
        { name: 'JT/T 1078', version: '2.7.4', desc: '部标 1078 UDP 流接入', enabled: true },
        { name: 'ZLM 客户端', version: '8.5', desc: 'ZLMediaKit HTTP API 封装', enabled: true },
        { name: '录像调度', version: '2.7.4', desc: '录像计划执行器', enabled: true },
        { name: '云录像', version: '2.7.4', desc: '对象存储归档', enabled: true },
        { name: 'JT 1078 补帧', version: '2.7.4', desc: '丢包后请求重传', enabled: false },
        { name: 'Redis 缓存', version: '7.2', desc: '热点数据缓存', enabled: true },
        { name: '告警通知', version: '2.7.4', desc: '邮件 / 短信 / Webhook', enabled: true },
        { name: 'API 鉴权', version: '2.7.4', desc: 'JWT + API Key', enabled: true },
        { name: '审计日志', version: '2.7.4', desc: '用户与系统行为记录', enabled: true }
      ],
      activity: [
        { t: '2026-07-05 16:30', text: '新建录像计划 海珠区 · 24×7 全周', by: 'admin' },
        { t: '2026-07-05 14:18', text: '配置变更：告警通知新增 Webhook', by: 'admin' },
        { t: '2026-07-05 12:00', text: '数据库 VACUUM + ANALYZE 自动执行', by: 'system' },
        { t: '2026-07-05 09:42', text: '新增媒体节点 zlm-edge-04 已加入', by: 'admin' },
        { t: '2026-07-05 08:00', text: '级联平台 上级交警支队 心跳正常', by: 'system' },
        { t: '2026-07-04 22:14', text: '删除设备 海珠仓库 3', by: 'ops-haizhu' }
      ]
    }
  }
}
</script>

<style lang="scss" scoped>
.kv { width: 100%; font-size: 12px; }
.kv .k { color: var(--text-tertiary); padding: 4px 0; }
.kv .v { text-align: right; }

.runtime { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 12px; }
.runtime__row { display: flex; justify-content: space-between; font-size: 12px; color: var(--text-secondary); margin-bottom: 4px; }

.module-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 10px; }
.module { padding: 10px 12px; background: var(--bg-base); border: 1px solid var(--border-subtle); border-radius: 6px; display: flex; flex-direction: column; gap: 4px; }
.module__name { font-size: 13px; font-weight: 600; }

.activity { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 8px; font-size: 12px; }
.activity li { padding: 6px 8px; background: var(--bg-base); border-radius: 4px; }
</style>
