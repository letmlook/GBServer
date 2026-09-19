<template>
  <!--
    平台接入信息：SIP / GB28181 与 JT1078 的对外接入参数。

    这些值 admin 要抄进设备或下级平台的配置界面里，所以全部明文展示
    （SIP 密码也是明文，后端在 JWT 之后才返回），并给每一项都配了复制按钮。
  -->
  <div class="platform-info">
    <section class="proto-block">
      <h4 class="proto-title">
        <el-icon :size="14"><Connection /></el-icon>
        <span>GB28181 / SIP</span>
        <span v-if="sipConfig?.enabled" class="health-pill ok">
          <i class="health-dot" /> 已启用
        </span>
        <span v-else class="health-pill err">
          <i class="health-dot" /> 已禁用
        </span>
      </h4>
      <dl v-if="sipConfig" class="proto-list">
        <div class="proto-row">
          <dt>SIP 服务器 ID</dt>
          <dd class="proto-value mono">{{ sipConfig.device_id }}
            <button class="proto-copy" title="复制 SIP 服务器 ID" @click="copy(sipConfig.device_id ?? '', 'SIP 服务器 ID')">
              <el-icon :size="12"><DocumentCopy /></el-icon>
            </button>
          </dd>
        </div>
        <div class="proto-row">
          <dt>SIP 服务器域</dt>
          <dd class="proto-value mono">{{ sipConfig.realm }}
            <button class="proto-copy" title="复制 SIP 服务器域" @click="copy(sipConfig.realm ?? '', 'SIP 服务器域')">
              <el-icon :size="12"><DocumentCopy /></el-icon>
            </button>
          </dd>
        </div>
        <div class="proto-row">
          <dt>SIP 用户名</dt>
          <dd class="proto-value mono">
            {{ sipConfig.username ?? sipConfig.device_id }}
            <button class="proto-copy" title="复制 SIP 用户名" @click="copy(sipConfig.username ?? sipConfig.device_id ?? '', 'SIP 用户名')">
              <el-icon :size="12"><DocumentCopy /></el-icon>
            </button>
          </dd>
        </div>
        <div class="proto-row">
          <dt>SIP 密码</dt>
          <dd class="proto-value mono">{{ sipConfig.password }}
            <button class="proto-copy" title="复制 SIP 密码" @click="copy(sipConfig.password ?? '', 'SIP 密码')">
              <el-icon :size="12"><DocumentCopy /></el-icon>
            </button>
          </dd>
        </div>
        <div class="proto-row">
          <dt>SIP 服务器地址</dt>
          <dd class="proto-value mono">{{ serverAddr }}
            <button class="proto-copy" title="复制 SIP 服务器地址" @click="copy(serverAddr, 'SIP 服务器地址')">
              <el-icon :size="12"><DocumentCopy /></el-icon>
            </button>
          </dd>
        </div>
        <div class="proto-row">
          <dt>监听</dt>
          <dd class="proto-value mono">
            {{ listenAddr(sipConfig.bind_ip || sipConfig.ip) }}:{{ sipConfig.port }}
            <span class="meta">{{ sipConfig.tcp_enabled ? 'UDP/TCP' : 'UDP' }}</span>
            <button class="proto-copy" title="复制监听地址" @click="copy(listenAddr(sipConfig.bind_ip || sipConfig.ip) + ':' + sipConfig.port, '监听地址')">
              <el-icon :size="12"><DocumentCopy /></el-icon>
            </button>
          </dd>
        </div>
        <div class="proto-row">
          <dt>心跳</dt>
          <dd class="proto-value mono">{{ sipConfig.keepalive_timeout }}s</dd>
        </div>
        <div class="proto-row">
          <dt>注册</dt>
          <dd class="proto-value mono">{{ sipConfig.register_timeout }}s</dd>
        </div>
        <div class="proto-row">
          <dt>字符集</dt>
          <dd class="proto-value mono">{{ sipConfig.charset }}</dd>
        </div>
      </dl>
      <div v-else class="proto-empty text-tertiary text-xs">未配置 SIP</div>
    </section>

    <section class="proto-block">
      <h4 class="proto-title">
        <el-icon :size="14"><VideoPlay /></el-icon>
        <span>JT1078 / 车载</span>
        <span v-if="jt1078Config" class="health-pill ok">
          <i class="health-dot" /> 已配置
        </span>
      </h4>
      <dl v-if="jt1078Config" class="proto-list">
        <div class="proto-row">
          <dt>TCP</dt>
          <dd class="proto-value mono">{{ listenAddr(null) }}:{{ jt1078Config.tcp_port }}
            <button class="proto-copy" title="复制 JT1078 TCP 地址" @click="copy(listenAddr(null) + ':' + jt1078Config.tcp_port, 'JT1078 TCP')">
              <el-icon :size="12"><DocumentCopy /></el-icon>
            </button>
          </dd>
        </div>
        <div class="proto-row">
          <dt>UDP</dt>
          <dd class="proto-value mono">{{ listenAddr(null) }}:{{ jt1078Config.udp_port }}
            <button class="proto-copy" title="复制 JT1078 UDP 地址" @click="copy(listenAddr(null) + ':' + jt1078Config.udp_port, 'JT1078 UDP')">
              <el-icon :size="12"><DocumentCopy /></el-icon>
            </button>
          </dd>
        </div>
        <div v-if="jt1078Config.timeout_ms" class="proto-row">
          <dt>会话超时</dt>
          <dd class="proto-value mono">{{ (jt1078Config.timeout_ms / 1000).toFixed(1) }}s</dd>
        </div>
        <div v-if="jt1078Config.retransmit_wait_ms" class="proto-row">
          <dt>重传等待</dt>
          <dd class="proto-value mono">{{ jt1078Config.retransmit_wait_ms }}ms</dd>
        </div>
        <div v-if="jt1078Config.retransmit_hook_url" class="proto-row">
          <dt>重传回调</dt>
          <dd class="proto-value mono">{{ jt1078Config.retransmit_hook_url }}
            <button class="proto-copy" title="复制重传回调 URL" @click="copy(jt1078Config.retransmit_hook_url ?? '', '重传回调')">
              <el-icon :size="12"><DocumentCopy /></el-icon>
            </button>
          </dd>
        </div>
      </dl>
      <div v-else class="proto-empty text-tertiary text-xs">未配置 JT1078</div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { ElMessage } from 'element-plus'
import { Connection, DocumentCopy, VideoPlay } from '@element-plus/icons-vue'
import type { SystemInfo } from '@/api/log'

const props = defineProps<{
  sipConfig?: SystemInfo['sip_config']
  jt1078Config?: SystemInfo['jt1078_config']
  /** 本机对外 IP（后端自动探测），用于把监听地址补全成可达地址 */
  hostIp?: string | null
}>()

/**
 * 监听地址：配置里写 `0.0.0.0`（通配）时回退到 host_ip，
 * 让 admin 看到真实可路由地址而不是通配符。
 */
function listenAddr(raw: string | null | undefined): string {
  if (raw && raw !== '0.0.0.0' && raw !== '::') return raw
  return props.hostIp ?? '0.0.0.0'
}

/** 设备/下级平台里要填的"SIP 服务器地址"（IP:端口） */
const serverAddr = computed(() => {
  const sip = props.sipConfig
  if (!sip) return '-'
  return `${listenAddr(sip.bind_ip || sip.ip)}:${sip.port}`
})

/**
 * 复制到剪贴板。`navigator.clipboard` 需要 HTTPS 或 localhost，
 * 部署在内网 IP 上时用 textarea + execCommand 兜底。
 */
async function copy(text: string, label = '') {
  const ok = await (async () => {
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text)
        return true
      }
    } catch {
      /* fall through */
    }
    try {
      const ta = document.createElement('textarea')
      ta.value = text
      ta.style.position = 'fixed'
      ta.style.opacity = '0'
      document.body.appendChild(ta)
      ta.select()
      const r = document.execCommand('copy')
      document.body.removeChild(ta)
      return r
    } catch {
      return false
    }
  })()
  ElMessage({
    message: ok ? `已复制${label ? ' ' + label : ''}` : '复制失败，请手动选择文本',
    type: ok ? 'success' : 'error',
    duration: 1500
  })
}
</script>

<style lang="scss" scoped>
.platform-info {
  display: flex;
  flex-direction: column;
  gap: 12px;
  /* 弹层里高度受限：内容多时内部滚动，不要把整个浮层撑出屏幕 */
  max-height: min(60vh, 560px);
  overflow-y: auto;
  padding-right: 2px;
}

.proto-block {
  padding: 10px 12px;
  background: var(--bg-elevated);
  border-radius: 6px;
}
.proto-title {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0 0 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);

  .health-pill { margin-left: auto; }
}
.proto-list {
  margin: 0;
  padding: 0;
  display: grid;
  grid-template-columns: 1fr;
  gap: 5px;
}
.proto-row {
  display: grid;
  grid-template-columns: 96px 1fr;
  gap: 10px;
  align-items: center;
  font-size: var(--text-xs);

  dt { color: var(--text-tertiary); margin: 0; }
  dd { color: var(--text-primary); margin: 0; word-break: break-all; }
  .meta { margin-left: 6px; color: var(--text-tertiary); font-weight: 400; }
}
.proto-value {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  flex-wrap: wrap;
}
.proto-copy {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 2px;
  background: transparent;
  border: 0;
  color: var(--text-tertiary);
  cursor: pointer;
  border-radius: 3px;
  transition: color 0.15s, background 0.15s;
  line-height: 0;

  &:hover { color: var(--brand-primary-500); background: rgba(11, 138, 178, 0.10); }
  &:active { transform: scale(0.9); }
}
.proto-empty { padding: 12px 0; text-align: center; }

.health-pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 2px 10px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 500;
  background: var(--bg-overlay);
  color: var(--text-tertiary);

  &.ok { color: var(--state-success); background: rgba(22, 163, 74, 0.10); }
  &.err { color: var(--state-error); background: rgba(220, 38, 38, 0.10); }

  .health-dot {
    width: 6px; height: 6px; border-radius: 50%;
    background: currentColor;
  }
}

.mono { font-family: var(--font-mono); }
.text-xs { font-size: var(--text-xs); }
.text-tertiary { color: var(--text-tertiary); }
</style>
