<template>
  <el-dialog
    :model-value="modelValue"
    @update:model-value="(v) => emit('update:modelValue', v)"
    :title="title"
    width="1180px"
    align-center
    destroy-on-close
    :close-on-click-modal="false"
    @open="onOpen"
    @close="onClose"
  >
    <div v-loading="loading" class="play-dialog">
      <!-- 第一行：左视频 + 右云台 -->
      <div class="play-dialog__row">
        <div class="play-dialog__stage">
          <!-- 协议切换条：拉流成功后展示所有可用协议作为 chip，
               当前正在播的协议高亮，点击即切换。
               长时间观看 WebRTC 偶发延迟劣化时，可一键切到 FLV/HLS。 -->
          <div v-if="availableProtocols.length > 0" class="protocol-switch">
            <span class="protocol-switch__label">协议</span>
            <div class="protocol-switch__chips">
              <button
                v-for="p in availableProtocols"
                :key="p.key"
                type="button"
                :class="['protocol-chip', `protocol-chip--${p.tone}`, { 'is-active': p.key === currentProtocol }]"
                :disabled="switchingProtocol === p.key"
                :title="p.description"
                @click="switchProtocol(p.key)"
              >
                <span class="protocol-chip__dot" />
                <span class="protocol-chip__name">{{ p.label }}</span>
                <span class="protocol-chip__latency">{{ p.latency }}</span>
              </button>
            </div>
            <span v-if="currentProtocol" class="protocol-switch__current mono">
              {{ protocolMeta(currentProtocol)?.label }}
            </span>
          </div>

          <div class="play-dialog__video">
            <video
              v-show="hasStream"
              ref="nativeRef"
              controls
              autoplay
              muted
              playsinline
              class="video-element native-hls"
              @error="onError"
              @loadeddata="onLoaded"
            />
            <video
              v-show="false"
              ref="flvRef"
              controls
              autoplay
              muted
              playsinline
              class="video-element flv-js"
            />
            <div v-if="!hasStream && !loading" class="play-dialog__placeholder">
              <el-icon :size="48"><VideoCameraFilled /></el-icon>
              <p class="placeholder-text">{{ statusLabel }}</p>
            </div>
            <div v-if="hasStream" class="play-dialog__live-badge">
              <span class="live-dot" />LIVE
            </div>
          </div>
          <div class="play-dialog__bar">
            <el-tag
              :type="playerStatus === 'playing' ? 'success' : playerStatus === 'error' ? 'danger' : 'info'"
              size="small"
              effect="dark"
            >
              {{ statusLabel }}
            </el-tag>
            <span class="mono play-dialog__id">
              {{ channel?.deviceId }} / {{ channel?.channelId }}
            </span>
            <el-button-group size="small">
              <el-button @click="onSnap">抓图</el-button>
              <el-button type="danger" plain @click="onStop">停止</el-button>
            </el-button-group>
          </div>
        </div>

        <!-- 云台面板：8 向 D-Pad + 速度 + 镜头 -->
        <section class="play-dialog__panel play-dialog__panel--ptz">
          <header class="panel-header">
            <span class="panel-title">云台控制</span>
            <span class="panel-meta">按下移动 · 松开停止</span>
          </header>

          <!-- 9 宫格 D-Pad：**一个** 3×3 grid，9 个格子按行顺序填充。
               之前拆成 3 个 .ptz-dpad__row（各自 3 列的 grid）会导致
               行与行之间列宽独立计算 + 外层 flex 居中产生累积偏移，
               视觉上右边对不齐。合成单个 grid 后 9 个格子由同一套
               grid-template 定义，严格对齐。 -->
          <div class="ptz-dpad">
            <el-button
              circle
              size="large"
              class="ptz-dpad__btn ptz-dpad__btn--dir"
              :icon="ArrowUpLeft"
              title="左上"
              @mousedown.prevent="startMove('UP_LEFT')"
              @mouseup.prevent="stopMove()"
              @mouseleave="stopMove()"
              @touchstart.prevent="startMove('UP_LEFT')"
              @touchend.prevent="stopMove()"
            />
            <el-button
              circle
              size="large"
              class="ptz-dpad__btn ptz-dpad__btn--dir"
              :icon="ArrowUp"
              title="向上"
              @mousedown.prevent="startMove('UP')"
              @mouseup.prevent="stopMove()"
              @mouseleave="stopMove()"
              @touchstart.prevent="startMove('UP')"
              @touchend.prevent="stopMove()"
            />
            <el-button
              circle
              size="large"
              class="ptz-dpad__btn ptz-dpad__btn--dir"
              :icon="ArrowUpRight"
              title="右上"
              @mousedown.prevent="startMove('UP_RIGHT')"
              @mouseup.prevent="stopMove()"
              @mouseleave="stopMove()"
              @touchstart.prevent="startMove('UP_RIGHT')"
              @touchend.prevent="stopMove()"
            />
            <el-button
              circle
              size="large"
              class="ptz-dpad__btn ptz-dpad__btn--dir"
              :icon="ArrowLeft"
              title="向左"
              @mousedown.prevent="startMove('LEFT')"
              @mouseup.prevent="stopMove()"
              @mouseleave="stopMove()"
              @touchstart.prevent="startMove('LEFT')"
              @touchend.prevent="stopMove()"
            />
            <span class="ptz-dpad__spacer" aria-hidden="true" />
            <el-button
              circle
              size="large"
              class="ptz-dpad__btn ptz-dpad__btn--dir"
              :icon="ArrowRight"
              title="向右"
              @mousedown.prevent="startMove('RIGHT')"
              @mouseup.prevent="stopMove()"
              @mouseleave="stopMove()"
              @touchstart.prevent="startMove('RIGHT')"
              @touchend.prevent="stopMove()"
            />
            <el-button
              circle
              size="large"
              class="ptz-dpad__btn ptz-dpad__btn--dir"
              :icon="ArrowDownLeft"
              title="左下"
              @mousedown.prevent="startMove('DOWN_LEFT')"
              @mouseup.prevent="stopMove()"
              @mouseleave="stopMove()"
              @touchstart.prevent="startMove('DOWN_LEFT')"
              @touchend.prevent="stopMove()"
            />
            <el-button
              circle
              size="large"
              class="ptz-dpad__btn ptz-dpad__btn--dir"
              :icon="ArrowDown"
              title="向下"
              @mousedown.prevent="startMove('DOWN')"
              @mouseup.prevent="stopMove()"
              @mouseleave="stopMove()"
              @touchstart.prevent="startMove('DOWN')"
              @touchend.prevent="stopMove()"
            />
            <el-button
              circle
              size="large"
              class="ptz-dpad__btn ptz-dpad__btn--dir"
              :icon="ArrowDownRight"
              title="右下"
              @mousedown.prevent="startMove('DOWN_RIGHT')"
              @mouseup.prevent="stopMove()"
              @mouseleave="stopMove()"
              @touchstart.prevent="startMove('DOWN_RIGHT')"
              @touchend.prevent="stopMove()"
            />
          </div>

          <!-- PTZ 运动速度 -->
          <div class="ptz-speed">
            <div class="ptz-speed__head">
              <span class="ptz-speed__label">运动速度</span>
              <span class="ptz-speed__value">
                <span class="mono">{{ ptzSpeed }}</span>
                <span class="ptz-speed__unit">/ {{ PTZ_SPEED_MAX }}</span>
              </span>
            </div>
            <el-slider
              v-model="ptzSpeed"
              :min="PTZ_SPEED_MIN"
              :max="PTZ_SPEED_MAX"
              :step="1"
              size="small"
              :marks="speedMarks"
              :format-tooltip="(v: number) => `速度 ${v}`"
              class="ptz-speed__slider"
            />
            <div class="ptz-speed__ticks">
              <span>1 慢</span>
              <span>4 中</span>
              <span>7 快</span>
            </div>
          </div>

          <!-- 镜头控制：放大/缩小 / 焦距± / 光圈±  三个分组 -->
          <div class="ptz-lens">
            <div class="ptz-lens__group">
              <span class="ptz-lens__label">变倍</span>
              <el-button-group size="default">
                <el-button
                  :icon="ZoomIn"
                  @mousedown.prevent="startMove('ZOOM_IN')"
                  @mouseup.prevent="stopMove()"
                  @mouseleave="stopMove()"
                  @touchstart.prevent="startMove('ZOOM_IN')"
                  @touchend.prevent="stopMove()"
                >放大</el-button>
                <el-button
                  :icon="ZoomOut"
                  @mousedown.prevent="startMove('ZOOM_OUT')"
                  @mouseup.prevent="stopMove()"
                  @mouseleave="stopMove()"
                  @touchstart.prevent="startMove('ZOOM_OUT')"
                  @touchend.prevent="stopMove()"
                >缩小</el-button>
              </el-button-group>
            </div>
            <div class="ptz-lens__group">
              <span class="ptz-lens__label">聚焦</span>
              <el-button-group size="default">
                <el-button
                  title="焦距+"
                  @mousedown.prevent="startMove('FOCUS_NEAR')"
                  @mouseup.prevent="stopMove()"
                  @mouseleave="stopMove()"
                  @touchstart.prevent="startMove('FOCUS_NEAR')"
                  @touchend.prevent="stopMove()"
                >远</el-button>
                <el-button
                  title="焦距-"
                  @mousedown.prevent="startMove('FOCUS_FAR')"
                  @mouseup.prevent="stopMove()"
                  @mouseleave="stopMove()"
                  @touchstart.prevent="startMove('FOCUS_FAR')"
                  @touchend.prevent="stopMove()"
                >近</el-button>
              </el-button-group>
            </div>
            <div class="ptz-lens__group">
              <span class="ptz-lens__label">光圈</span>
              <el-button-group size="default">
                <el-button
                  title="光圈+"
                  @mousedown.prevent="startMove('IRIS_OPEN')"
                  @mouseup.prevent="stopMove()"
                  @mouseleave="stopMove()"
                  @touchstart.prevent="startMove('IRIS_OPEN')"
                  @touchend.prevent="stopMove()"
                >开</el-button>
                <el-button
                  title="光圈-"
                  @mousedown.prevent="startMove('IRIS_CLOSE')"
                  @mouseup.prevent="stopMove()"
                  @mouseleave="stopMove()"
                  @touchstart.prevent="startMove('IRIS_CLOSE')"
                  @touchend.prevent="stopMove()"
                >关</el-button>
              </el-button-group>
            </div>
          </div>
        </section>
      </div>

      <!-- 第二行：协议地址，跨满全宽 -->
      <section class="play-dialog__panel play-dialog__panel--urls">
        <header class="panel-header">
          <span class="panel-title">协议地址</span>
          <span v-if="urls.length" class="panel-meta">{{ urls.length }} 路</span>
        </header>
        <p v-if="!urls.length" class="play-dialog__hint">
          <el-icon><InfoFilled /></el-icon> 尚未拉起流，先点「播放」
        </p>
        <ul v-else class="play-dialog__urls">
          <li v-for="u in urls" :key="u.label" class="url-row">
            <span :class="['gb-chip', `gb-chip--${u.tone}`]">{{ u.label }}</span>
            <el-tooltip
              :content="u.url"
              placement="top"
              :show-after="100"
              :disabled="!u.url"
            >
              <el-input
                :model-value="u.url"
                readonly
                size="small"
                class="url-row__input"
                :title="u.url"
                @focus="selectAll($event)"
              >
                <template #append>
                  <el-button
                    size="small"
                    :icon="CopyDocument"
                    @click="copyUrl(u.url, u.label)"
                  >复制</el-button>
                </template>
              </el-input>
            </el-tooltip>
          </li>
        </ul>
      </section>
    </div>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, h, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import {
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  CopyDocument,
  InfoFilled,
  VideoCameraFilled,
  ZoomIn,
  ZoomOut
} from '@element-plus/icons-vue'

// 4 个斜向箭头：Element Plus icons 没现成的，自绘 SVG 当 :icon 用。
// viewBox=0 0 24 24 与 Element Plus 的 chevron 保持一致。
//
// 路径写法要点：**杆的两个端点必须与箭头所在的角重合**。
// 上一版把「左上箭头」写成 `M17 7L7 17`（从右上拉到左下）+ 箭头在 (7,7)，
// 杆的起点是右上、箭头在左上 —— 两者不共角，渲染出来是"一根斜线 + 一个
// 悬空的 L 角"，看着就像歪掉的小三角。正确写法是杆从对角 (17,17) 指向
// 箭头所在角 (7,7)，头部用 `V/H` 折线围成 L 形。
const ArrowUpLeft = () =>
  h(
    'svg',
    { viewBox: '0 0 24 24', width: 22, height: 22, fill: 'none' },
    [
      h('path', {
        // 杆：右下 (17,17) → 左上 (7,7)；箭头：竖线 (7,14→7,7) + 横线 (7,7→14,7)
        d: 'M17 17L7 7M7 14V7H14',
        stroke: 'currentColor',
        'stroke-width': '2',
        'stroke-linecap': 'round',
        'stroke-linejoin': 'round'
      })
    ]
  )
const ArrowUpRight = () =>
  h(
    'svg',
    { viewBox: '0 0 24 24', width: 22, height: 22, fill: 'none' },
    [
      h('path', {
        // 杆：左下 (7,17) → 右上 (17,7)；箭头：竖线 (17,14→17,7) + 横线 (17,7→10,7)
        d: 'M7 17L17 7M17 14V7H10',
        stroke: 'currentColor',
        'stroke-width': '2',
        'stroke-linecap': 'round',
        'stroke-linejoin': 'round'
      })
    ]
  )
const ArrowDownLeft = () =>
  h(
    'svg',
    { viewBox: '0 0 24 24', width: 22, height: 22, fill: 'none' },
    [
      h('path', {
        // 杆：右上 (17,7) → 左下 (7,17)；箭头：竖线 (7,10→7,17) + 横线 (7,17→14,17)
        d: 'M17 7L7 17M7 10V17H14',
        stroke: 'currentColor',
        'stroke-width': '2',
        'stroke-linecap': 'round',
        'stroke-linejoin': 'round'
      })
    ]
  )
const ArrowDownRight = () =>
  h(
    'svg',
    { viewBox: '0 0 24 24', width: 22, height: 22, fill: 'none' },
    [
      h('path', {
        // 杆：左上 (7,7) → 右下 (17,17)；箭头：竖线 (17,10→17,17) + 横线 (17,17→10,17)
        d: 'M7 7L17 17M17 10V17H10',
        stroke: 'currentColor',
        'stroke-width': '2',
        'stroke-linecap': 'round',
        'stroke-linejoin': 'round'
      })
    ]
  )
import { ElMessage } from 'element-plus'
import {
  playSnap,
  postWebrtcPlay,
  sendPtz as sendPtzApi,
  startPlay,
  stopPlay,
  type PlayStartResult
} from '@/api/live'

interface Channel {
  deviceId: string
  channelId: string
  name?: string
}

const props = defineProps<{
  modelValue: boolean
  channel: Channel | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  /** 抓图结果。`auto=true` 表示拉流成功后自动抓的缩略图。 */
  (e: 'snap', snapUrl: string, payload?: { auto?: boolean }): void
}>()

const loading = ref(false)
const playerStatus = ref<'idle' | 'loading' | 'playing' | 'error'>('idle')
const nativeRef = ref<HTMLVideoElement | null>(null)
const flvRef = ref<HTMLVideoElement | null>(null)
const playData = ref<PlayStartResult | null>(null)

/**
 * 协议切换：
 * - availableProtocols：startPlay 成功后从 playData 派生可用协议 + 估时延迟
 * - currentProtocol：当前正在播放的协议 key（'webrtc' / 'hls' / 'flv' / 'ws_flv' / 'rtsp' 等）
 * - switchingProtocol：用户点了 chip 但还没完成切换的 key —— 用于 chip 上的
 *   loading 反馈，避免重复点击
 * - 切换协议时复用 startPlay 的结果，**不再重新发 SIP INVITE**（后端会把
 *   同一路流同时 push 到 ZLM 多协议上），只在客户端切渲染。
 */
type ProtocolKey = 'webrtc' | 'hls' | 'flv' | 'ws_flv' | 'ws' | 'rtsp' | 'rtmp'
const currentProtocol = ref<ProtocolKey | ''>('')
const switchingProtocol = ref<ProtocolKey | ''>('')

// PTZ 运动速度：协议上是单字节 0-255，但**主流厂商（Hikvision/Dahua/宇视等）
// 实际只取 1-7 这 7 个档位**（0 留给"停止"语义）。GB28181 标准 DeviceConfig
// 没有专门的下发端点返回设备的 PTZ 速度范围，因此采用业界默认 1-7 区间，
// 默认 4 是"中速"，向前兼容绝大多数国标设备。
//
// 未来如需按设备适配：可新增 `GET /api/device/ptz-speed-range/{deviceId}`，
// 返回 {min, max, default}；前端打开对话框时拉一次覆盖即可。
const PTZ_SPEED_MIN = 1
const PTZ_SPEED_MAX = 7
const PTZ_SPEED_DEFAULT = 4
const ptzSpeed = ref(PTZ_SPEED_DEFAULT)
const speedMarks = computed<Record<number, string>>(() => ({
  1: '1',
  4: '4',
  7: '7'
}))

let hlsInstance: any = null
let flvPlayer: any = null
let webrtcPc: RTCPeerConnection | null = null

/**
 * 云台方向键按下/抬起状态：
 * - ptzMoveCmd：当前正在持续发送的方向命令；非空说明有键按下
 * - ptzMoveTimer：每 250ms 续发一次 MOVE 指令，让设备的 PTZ 机芯保持转动；
 *   大多数球机/云台需要周期性命令，单次 SIP MESSAGE 只能让它动一下就停
 * - startMove()/stopMove() 由 mousedown/mouseup/touchstart/touchend 触发
 */
let ptzMoveCmd = ''
let ptzMoveTimer: number | null = null
const PTZ_MOVE_INTERVAL_MS = 250

const title = computed(() => {
  if (!props.channel) return '通道播放'
  const nm = props.channel.name || props.channel.channelId
  return `${nm} · ${props.channel.deviceId}/${props.channel.channelId}`
})

const statusLabel = computed(() => {
  switch (playerStatus.value) {
    case 'loading': return '加载中…'
    case 'playing': return '● 播放中'
    case 'error':   return '播放失败'
    default:        return '空闲'
  }
})

const hasStream = computed(() => playerStatus.value !== 'idle')

interface UrlRow {
  label: string
  url: string
  tone: 'success' | 'warning' | 'info' | 'primary'
}

/**
 * 把后端返回的 PlayStartResult 拍平成「协议 → URL」列表。
 * 后端一次性给齐 HLS / FLV / WS-FLV / WebRTC / RTSP / RTMP，
 * 这里按"播放前端最常用 → 最不常用"排序，便于复制。
 */
const urls = computed<UrlRow[]>(() => {
  const d = playData.value
  if (!d) return []
  const out: UrlRow[] = []
  if (d.webrtc) out.push({ label: 'WebRTC', url: d.webrtc, tone: 'success' })
  if (d.hls) out.push({ label: 'HLS', url: d.hls, tone: 'success' })
  if (d.flvUrl) out.push({ label: 'HTTP-FLV', url: d.flvUrl, tone: 'success' })
  if (d.ws_flv) out.push({ label: 'WS-FLV', url: d.ws_flv, tone: 'success' })
  if (d.wsUrl) out.push({ label: 'WS', url: d.wsUrl, tone: 'info' })
  if (d.playUrl) {
    const scheme = d.playUrl.startsWith('rtsp://') ? 'RTSP'
      : d.playUrl.startsWith('rtmp://') ? 'RTMP'
      : '播放'
    out.push({ label: scheme, url: d.playUrl, tone: 'warning' })
  }
  return out
})

interface ProtocolOption {
  key: ProtocolKey
  label: string
  /** 估计延迟：WebRTC 最低，HTTP-FLV 中，HLS 高但稳，RTSP/RTMP 浏览器原生不支持 */
  latency: string
  description: string
  tone: 'success' | 'warning' | 'info'
}

/** 与 urls 列表配套：协议 chip 一栏的元信息（标签 / 延迟 / 描述） */
const protocolCatalog: Record<ProtocolKey, ProtocolOption> = {
  webrtc: { key: 'webrtc', label: 'WebRTC', latency: '低延迟', description: 'WebRTC（最低延迟，同子网直连）', tone: 'success' },
  hls:    { key: 'hls',    label: 'HLS',    latency: '~5s',   description: 'HLS m3u8 切片（最稳定，延迟较高）', tone: 'info' },
  flv:    { key: 'flv',    label: 'HTTP-FLV', latency: '~1s', description: 'HTTP-FLV 流式（低延迟 + 抗抖动）', tone: 'success' },
  ws_flv: { key: 'ws_flv', label: 'WS-FLV', latency: '~1s',  description: 'WebSocket 封装的 FLV（绕过 HTTP 代理）', tone: 'info' },
  ws:     { key: 'ws',     label: 'WS',     latency: '~5s',   description: 'WebSocket 通用流', tone: 'info' },
  rtsp:   { key: 'rtsp',   label: 'RTSP',   latency: '原生', description: 'RTSP 直连（浏览器原生不支持，需插件）', tone: 'warning' },
  rtmp:   { key: 'rtmp',   label: 'RTMP',   latency: '原生', description: 'RTMP 直连（浏览器原生不支持）', tone: 'warning' }
}

/** 从 playData 推导出当前可用的协议 chip 列表 */
const availableProtocols = computed<ProtocolOption[]>(() => {
  const d = playData.value
  if (!d) return []
  const out: ProtocolOption[] = []
  if (d.webrtc)  out.push(protocolCatalog.webrtc)
  if (d.hls)     out.push(protocolCatalog.hls)
  if (d.flvUrl)  out.push(protocolCatalog.flv)
  if (d.ws_flv)  out.push(protocolCatalog.ws_flv)
  if (d.wsUrl)   out.push(protocolCatalog.ws)
  if (d.playUrl) {
    if (d.playUrl.startsWith('rtsp://')) out.push(protocolCatalog.rtsp)
    else if (d.playUrl.startsWith('rtmp://')) out.push(protocolCatalog.rtmp)
  }
  return out
})

function protocolMeta(key: ProtocolKey): ProtocolOption | undefined {
  return protocolCatalog[key]
}

function selectAll(ev: FocusEvent) {
  const t = ev.target as HTMLInputElement | null
  t?.select()
}

async function copyUrl(url: string, label: string) {
  try {
    await navigator.clipboard.writeText(url)
    ElMessage.success(`已复制 ${label}`)
  } catch {
    const ta = document.createElement('textarea')
    ta.value = url
    ta.style.position = 'fixed'
    ta.style.opacity = '0'
    document.body.appendChild(ta)
    ta.select()
    document.execCommand('copy')
    document.body.removeChild(ta)
    ElMessage.success(`已复制 ${label}`)
  }
}

async function onOpen() {
  if (!props.channel) return
  await playChannel()
}

async function onClose() {
  // 关对话框时一定要先停云台，避免用户按住后关闭弹窗让设备一直转动
  stopMove()
  await stopChannel()
}

watch(
  () => props.modelValue,
  (v) => {
    if (!v) {
      stopMove()
      stopChannel()
    }
  }
)

onBeforeUnmount(() => {
  stopMove()
  stopChannel()
})

async function playChannel() {
  if (!props.channel) return
  const { deviceId, channelId } = props.channel
  playerStatus.value = 'loading'
  loading.value = true
  try {
    const res = await startPlay(deviceId, channelId)
    const data = res.data as PlayStartResult
    if (!data) throw new Error(res.msg || '拉起实时流失败')
    playData.value = data
    await nextTick()
    // 默认按"低延迟优先"：WebRTC → HLS → HTTP-FLV → RTSP/RTMP 链式兜底
    if (data.webrtc) {
      try {
        await attachWebRTC(deviceId, channelId)
        currentProtocol.value = 'webrtc'
      } catch (e: any) {
        cleanupWebRTC()
        ElMessage.warning(`WebRTC 协商失败，回退到 HLS/FLV：${e?.message ?? e}`)
      }
    }
    if (!currentProtocol.value) {
      const url = data.hls || data.flvUrl || data.playUrl || ''
      if (!url) throw new Error('后端未返回可用的播放地址')
      await attachVideo(url)
      currentProtocol.value = data.hls ? 'hls' : data.flvUrl ? 'flv' : data.playUrl.startsWith('rtsp') ? 'rtsp' : 'rtmp'
    }
    void autoSnapOnPlay(deviceId, channelId)
  } catch (e: any) {
    const msg = e?.message ?? '拉起实时流失败'
    playerStatus.value = 'error'
    ElMessage.error(`播放失败：${msg}`)
  } finally {
    loading.value = false
  }
}

/**
 * 手动切换协议（用户在 chip 栏点击）：
 * - 复用 playData 里的 URL，不再调 startPlay（避免触发后端重新发 SIP INVITE）
 * - 切到 WebRTC 走 attachWebRTC；切到其他走 attachVideo
 * - 切换期间显示 chip 上的 loading 反馈（switchingProtocol）
 */
async function switchProtocol(key: ProtocolKey) {
  if (!props.channel || !playData.value || switchingProtocol.value) return
  if (key === currentProtocol.value) return
  const data = playData.value
  const { deviceId, channelId } = props.channel
  switchingProtocol.value = key
  playerStatus.value = 'loading'
  try {
    // 先清掉当前播放器和 WebRTC PeerConnection
    destroyPlayers()
    cleanupWebRTC()
    if (key === 'webrtc') {
      if (!data.webrtc) {
        ElMessage.warning('当前通道未提供 WebRTC 地址')
        return
      }
      await attachWebRTC(deviceId, channelId)
    } else {
      const url =
        key === 'hls'    ? data.hls :
        key === 'flv'    ? data.flvUrl :
        key === 'ws_flv' ? data.ws_flv :
        key === 'ws'     ? data.wsUrl :
        key === 'rtsp' || key === 'rtmp' ? data.playUrl : ''
      if (!url) {
        ElMessage.warning('该协议地址为空')
        return
      }
      await attachVideo(url)
    }
    currentProtocol.value = key
    ElMessage.success(`已切换到 ${protocolCatalog[key].label}`)
  } catch (e: any) {
    const msg = e?.message ?? `切换到 ${key} 失败`
    ElMessage.error(msg)
  } finally {
    switchingProtocol.value = ''
  }
}

async function autoSnapOnPlay(deviceId: string, channelId: string) {
  try {
    const res = await playSnap(deviceId, channelId)
    const url = res?.data?.snapUrl
    if (url) emit('snap', url, { auto: true })
  } catch {
    // 静默
  }
}

async function attachWebRTC(deviceId: string, channelId: string) {
  cleanupWebRTC()
  destroyPlayers()
  const pc = new RTCPeerConnection({ iceServers: [] })
  pc.addTransceiver('video', { direction: 'recvonly' })
  pc.addTransceiver('audio', { direction: 'recvonly' })
  const video = nativeRef.value
  if (!video) throw new Error('video 元素未挂载')
  pc.ontrack = (ev) => {
    if (ev.streams && ev.streams[0]) {
      video.srcObject = ev.streams[0]
      video.play().catch(() => {})
      playerStatus.value = 'playing'
    }
  }
  pc.oniceconnectionstatechange = () => {
    if (pc.iceConnectionState === 'failed' || pc.iceConnectionState === 'disconnected') {
      playerStatus.value = 'error'
      ElMessage.error(`WebRTC ICE ${pc.iceConnectionState}`)
    }
  }
  const offer = await pc.createOffer()
  await pc.setLocalDescription(offer)
  const res = await postWebrtcPlay({
    deviceId, channelId,
    sdp: offer.sdp ?? '',
    type: 'offer'
  })
  if (res.code !== 0) {
    pc.close()
    throw new Error(res.msg || 'WebRTC 协商失败')
  }
  await pc.setRemoteDescription({ type: 'answer', sdp: res.data.sdp })
  webrtcPc = pc
}

function cleanupWebRTC() {
  if (webrtcPc) {
    try { webrtcPc.close() } catch { /* ignore */ }
    webrtcPc = null
  }
  const v = nativeRef.value
  if (v) {
    try {
      const ms = v.srcObject as MediaStream | null
      if (ms) ms.getTracks().forEach((t) => t.stop())
    } catch { /* ignore */ }
    v.srcObject = null
  }
}

function destroyPlayers() {
  try { hlsInstance?.destroy?.() } catch {}
  hlsInstance = null
  try { flvPlayer?.destroy?.() } catch {}
  flvPlayer = null
}

async function attachVideo(url: string) {
  destroyPlayers()
  const video = nativeRef.value
  const flvVideo = flvRef.value
  if (!video) return
  // Safari 原生 HLS
  if (video.canPlayType('application/vnd.apple.mpegurl') !== '' && url.includes('.m3u8')) {
    video.src = url
    return
  }
  if (url.includes('.m3u8')) {
    try {
      const Hls = (await import('hls.js')).default
      if (Hls.isSupported()) {
        const hls = new Hls({ enableWorker: true, liveSyncDuration: 1 })
        hls.loadSource(url)
        hls.attachMedia(video)
        hls.on(Hls.Events.ERROR, (_e: any, data: any) => {
          if (data?.fatal) {
            playerStatus.value = 'error'
            ElMessage.error(`HLS 错误：${data?.details ?? data?.type ?? 'unknown'}`)
          }
        })
        hlsInstance = hls
        return
      }
    } catch { /* fall through */ }
  }
  if (url.includes('.flv') && flvjs_supported() && flvVideo) {
    try {
      const flvjs = (await import('flv.js')).default
      const player = flvjs.createPlayer({ type: 'flv', url, isLive: true })
      player.attachMediaElement(flvVideo)
      try { player.load() } catch {}
      const p = player.play()
      if (p && typeof (p as any).catch === 'function') (p as Promise<void>).catch(() => {})
      flvPlayer = player
      return
    } catch { /* fall through */ }
  }
  video.src = url
}

function flvjs_supported() {
  return typeof (window as any).MediaSource !== 'undefined'
}

function onLoaded() {
  playerStatus.value = 'playing'
}

function onError() {
  playerStatus.value = 'error'
  ElMessage.error('视频流加载失败')
}

async function onStop() {
  await stopChannel()
  playerStatus.value = 'idle'
  playData.value = null
  ElMessage.success('已停止')
}

async function stopChannel() {
  destroyPlayers()
  cleanupWebRTC()
  const v = nativeRef.value
  if (v) {
    try { v.pause() } catch {}
    v.removeAttribute('src')
    v.load()
  }
  if (props.channel) {
    await stopPlay(props.channel.deviceId, props.channel.channelId).catch(() => {})
  }
}

async function sendPtz(cmd: string) {
  if (!props.channel) return
  try {
    // 三路速度都用滑块当前值；horizon/vertical/zoom 各传一份让设备的 PTZ
    // 机芯按各轴独立解码（多数球机按同一速度执行）。
    await sendPtzApi({
      deviceId: props.channel.deviceId,
      channelId: props.channel.channelId,
      cmd,
      speed: ptzSpeed.value,
      horizonSpeed: ptzSpeed.value,
      verticalSpeed: ptzSpeed.value,
      zoomSpeed: ptzSpeed.value
    })
    // 仅单次命令弹 toast；持续移动的 PTZ_MOVE_INTERVAL_MS 续发不再弹，
    // 否则会刷屏干扰用户操作。
    if (!ptzMoveCmd) {
      ElMessage.success(`PTZ ${cmd} 已下发（速度 ${ptzSpeed.value}）`)
    }
  } catch (e: any) {
    ElMessage.error(e?.message ?? `PTZ ${cmd} 失败`)
  }
}

/**
 * 按下方向键：立即发一次 MOVE 命令，然后每 PTZ_MOVE_INTERVAL_MS 续发，
 * 模拟真实摇杆手感（按住期间云台一直转，松手才停）。
 *
 * 同一时刻只能有一个方向命令在跑；如果用户在 STOP 之前切换方向，
 * 直接覆盖 ptzMoveCmd，旧的定时器下次触发就发新方向，不需要 clearInterval。
 */
function startMove(cmd: string) {
  if (!props.channel) return
  ptzMoveCmd = cmd
  void sendPtz(cmd)
  if (ptzMoveTimer === null) {
    ptzMoveTimer = window.setInterval(() => {
      if (ptzMoveCmd) void sendPtz(ptzMoveCmd)
    }, PTZ_MOVE_INTERVAL_MS)
  }
}

/**
 * 松开方向键（或鼠标移出按钮、对话框关闭、组件卸载）：发 STOP，
 * 并清掉续发定时器。如果连续切换了多个方向（ptzMoveCmd 早已不等于
 * 触发 stopMove 时的 cmd），仍发 STOP 让设备立即停下来。
 */
/**
 * 松开方向键（或鼠标移出按钮、对话框关闭、组件卸载）：
 * **只有当 `ptzMoveCmd` 非空时**（即确实有键按下过）才向设备发 STOP。
 *
 * 修正：此前无条件 sendPtz('STOP') 会导致"打开对话框时光标扫过按钮
 * 触发 mouseleave → 误发 STOP" —— 出现一连串"PTZ STOP 已下发"toast，
 * 同时也违反"打开只读取状态不执行动作"的需求。
 *
 * 真正的"用户在松开"场景：startMove 先把 ptzMoveCmd 置为方向命令，
 * stopMove 检测到非空 → 发 STOP；打开对话框、对话框关闭等其他场景
 * ptzMoveCmd 始终是空串 → 不发任何命令。
 */
function stopMove() {
  const wasMoving = ptzMoveCmd !== ''
  ptzMoveCmd = ''
  if (ptzMoveTimer !== null) {
    window.clearInterval(ptzMoveTimer)
    ptzMoveTimer = null
  }
  if (wasMoving && props.channel) void sendPtz('STOP')
}

async function onSnap() {
  if (!props.channel) return
  try {
    const res = await playSnap(props.channel.deviceId, props.channel.channelId)
    const url = res.data?.snapUrl ?? ''
    if (url) emit('snap', url)
    ElMessage.success('抓图已保存')
  } catch (e: any) {
    ElMessage.error(e?.message ?? '抓图失败')
  }
}
</script>

<style lang="scss" scoped>
/* 整体两行布局：
   1. 第一行：视频 (左 65%) + 云台面板 (右 35%)  横向并排
   2. 第二行：协议地址面板，跨满全宽
   视觉上「视频下面挂着协议」+ 「云台始终在视频右侧触手可及」。 */
.play-dialog {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-height: 520px;
}
.play-dialog__row {
  display: grid;
  grid-template-columns: minmax(620px, 1fr) 360px;
  gap: 16px;
  align-items: stretch;
}

/* ---- 左：视频区 ---- */
.play-dialog__stage {
  display: flex; flex-direction: column; gap: 10px;
  min-width: 0;
}

/* ---- 协议切换条：横向 chip 栏，紧贴视频上方 ---- */
.protocol-switch {
  display: flex; align-items: center; gap: 10px;
  padding: 8px 12px;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
  flex: 0 0 auto;
}
.protocol-switch__label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
  letter-spacing: 0.3px;
  flex: 0 0 auto;
}
.protocol-switch__chips {
  display: flex; flex-wrap: wrap; gap: 6px;
  flex: 1 1 auto;
}
.protocol-switch__current {
  font-size: 12px;
  color: var(--brand-primary-500);
  flex: 0 0 auto;
}

.protocol-chip {
  display: inline-flex; align-items: center; gap: 6px;
  padding: 4px 10px;
  border-radius: 999px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
  user-select: none;
}
.protocol-chip:hover {
  border-color: var(--brand-primary-400);
  color: var(--brand-primary-500);
  background: rgba(11, 138, 178, 0.06);
}
.protocol-chip:active { transform: scale(0.96); }
.protocol-chip:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.protocol-chip__dot {
  width: 6px; height: 6px;
  border-radius: 50%;
  background: currentColor;
  opacity: 0.6;
}
/* 不同协议的色调：成功绿 / 信息蓝 / 警告黄 */
.protocol-chip--success .protocol-chip__dot { background: var(--state-success); opacity: 1; }
.protocol-chip--info    .protocol-chip__dot { background: var(--brand-primary-500); opacity: 1; }
.protocol-chip--warning .protocol-chip__dot { background: var(--state-warning); opacity: 1; }

.protocol-chip__name {
  font-weight: 600;
}
.protocol-chip__latency {
  font-size: 10px;
  color: var(--text-tertiary);
  font-weight: 500;
  padding: 1px 6px;
  background: var(--bg-overlay);
  border-radius: 999px;
}
/* 当前正在播放的协议：主色高亮 */
.protocol-chip.is-active {
  background: var(--brand-primary-500);
  border-color: var(--brand-primary-500);
  color: #fff;
}
.protocol-chip.is-active .protocol-chip__latency {
  background: rgba(255, 255, 255, 0.20);
  color: rgba(255, 255, 255, 0.9);
}
.protocol-chip.is-active .protocol-chip__dot {
  background: #fff;
  animation: protocol-pulse 1.4s ease-in-out infinite;
}
@keyframes protocol-pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50%      { opacity: 0.4; transform: scale(0.7); }
}
.play-dialog__video {
  position: relative;
  width: 100%;
  aspect-ratio: 16 / 9;
  background: linear-gradient(180deg, #0a0a0a 0%, #161616 100%);
  border-radius: 8px;
  overflow: hidden;
  display: flex; align-items: center; justify-content: center;
  border: 1px solid var(--border-default);
}
.video-element { width: 100%; height: 100%; object-fit: contain; background: #000; }
.play-dialog__placeholder {
  color: #666; text-align: center;
  display: flex; flex-direction: column; align-items: center; gap: 8px;
  .placeholder-text { font-size: 13px; opacity: 0.8; }
}
.play-dialog__live-badge {
  position: absolute;
  top: 12px; left: 12px;
  display: flex; align-items: center; gap: 6px;
  padding: 4px 10px;
  background: rgba(220, 38, 38, 0.92);
  color: #fff;
  font-size: 12px;
  font-weight: 600;
  border-radius: 4px;
  letter-spacing: 0.5px;
  .live-dot {
    width: 8px; height: 8px; border-radius: 50%;
    background: #fff;
    animation: live-pulse 1.4s ease-in-out infinite;
  }
}
@keyframes live-pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50%      { opacity: 0.4; transform: scale(0.85); }
}
.play-dialog__bar {
  display: flex; align-items: center; gap: 12px;
  padding: 8px 12px;
  background: var(--bg-elevated);
  border-radius: 6px;
  border: 1px solid var(--border-subtle);
}
.play-dialog__id {
  flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  color: var(--text-secondary);
  font-size: 12px;
}

/* ---- 三个面板（PTZ / 协议 URL）的通用样式 ---- */
.play-dialog__panel {
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  padding: 14px;
  background: var(--bg-surface);
  display: flex; flex-direction: column;
  min-width: 0;
}
/* PTZ 面板：允许 D-Pad 占满内容，并让内部分块自然伸展 */
.play-dialog__panel--ptz { min-height: 100%; }
.play-dialog__panel--urls { padding-bottom: 16px; }
.play-dialog__hint {
  display: flex; align-items: center; gap: 6px;
  color: var(--text-tertiary);
  font-size: 12px;
  margin: 0;
  padding: 8px 0;
}

.panel-header {
  display: flex; justify-content: space-between; align-items: center;
  margin-bottom: 10px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--border-subtle);
}
.panel-title {
  font-weight: 600;
  font-size: 13px;
  color: var(--text-primary);
  letter-spacing: 0.3px;
}
.panel-meta {
  font-size: 11px;
  color: var(--text-tertiary);
}

/* 协议地址列表：现在跨满全宽，用两列网格把 WebRTC/HLS/FLV/WS-FLV/WS/RTSP
   这 6 个协议平铺开来，单列纵向堆叠在宽屏下会浪费水平空间。 */
.play-dialog__urls {
  list-style: none; margin: 0; padding: 0;
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(440px, 1fr));
  gap: 10px;
  overflow-y: auto;
}
.url-row {
  display: grid;
  grid-template-columns: 78px 1fr;
  gap: 8px;
  align-items: center;
}
.url-row__input :deep(.el-input__inner) {
  font-family: var(--font-mono);
  font-size: 11px;
}

.gb-chip {
  display: inline-flex; align-items: center; justify-content: center;
  padding: 3px 0;
  border-radius: 4px;
  font-size: 11px; font-weight: 600;
  background: var(--bg-overlay); color: var(--text-secondary);
  letter-spacing: 0.3px;
}
.gb-chip--success { color: var(--state-success); background: rgba(22, 163, 74, 0.12); }
.gb-chip--warning { color: var(--state-warning); background: rgba(217, 119, 6, 0.12); }
.gb-chip--primary { color: var(--brand-primary-500); background: rgba(11, 138, 178, 0.12); }

/* ---- 云台：D-Pad + 镜头控制 ---- */
.ptz-dpad {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 8px 0 14px;
}
/* 9 宫格 D-Pad：单个 3×3 grid，9 个格子由同一套 track 定义 —— 严格对齐。
   justify-content: center 让整块（3×56 + 2×gap = 180px）在面板里水平居中；
   之前拆成 3 个独立 grid row，各行的列起点会因外层 flex 居中 + 各自宽度
   而漂移，右边因此对不齐。 */
.ptz-dpad {
  display: grid;
  grid-template-columns: repeat(3, 56px);
  grid-template-rows: repeat(3, 56px);
  gap: 6px;
  justify-content: center;
  width: 100%;
  padding: 8px 0 14px;
}
/* 中心格留空（9 宫格正中央） */
.ptz-dpad__spacer {
  display: block;
  width: 56px;
  height: 56px;
}

.ptz-dpad__btn {
  width: 56px;
  height: 56px;
  padding: 0;
  font-size: 18px;
}
/* 让按钮内的 SVG 严格居中：el-button 默认有 padding，会把 svg 挤出几何中心；
   这里强制 grid 居中 + svg 自身尺寸与 box 对齐 */
.ptz-dpad__btn :deep(.el-icon),
.ptz-dpad__btn :deep(svg) {
  width: 22px !important;
  height: 22px !important;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.ptz-dpad__btn :deep(.el-icon svg) {
  width: 22px;
  height: 22px;
}

.ptz-dpad__btn--dir {
  background: var(--bg-elevated);
  border-color: var(--border-default);
  color: var(--text-primary);
  transition: transform 0.1s, background 0.15s, border-color 0.15s;
}
.ptz-dpad__btn--dir:hover {
  background: rgba(11, 138, 178, 0.12);
  border-color: var(--brand-primary-400);
  color: var(--brand-primary-500);
  transform: translateY(-1px);
}
.ptz-dpad__btn--dir:active {
  transform: scale(0.95);
}
/* 按住期间持续移动：背景切到主色，让用户清楚知道正在转动 */
.ptz-dpad__btn--dir:active,
.ptz-dpad__btn--dir.is-pressing {
  background: rgba(11, 138, 178, 0.18);
  border-color: var(--brand-primary-500);
  color: var(--brand-primary-500);
  transform: scale(0.95);
}

/* ---- PTZ 速度滑块：D-Pad 与镜头组之间的分隔带 ---- */
.ptz-speed {
  margin: 4px 0 12px;
  padding: 10px 0 4px;
  border-top: 1px dashed var(--border-subtle);
  border-bottom: 1px dashed var(--border-subtle);
}
.ptz-speed__head {
  display: flex; justify-content: space-between; align-items: baseline;
  margin-bottom: 4px;
}
.ptz-speed__label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary);
}
.ptz-speed__value {
  display: inline-flex; align-items: baseline; gap: 4px;
  font-size: 13px;
  color: var(--brand-primary-500);
}
.ptz-speed__value .mono {
  font-size: 15px;
  font-weight: 600;
}
.ptz-speed__unit {
  font-size: 11px;
  color: var(--text-tertiary);
}
.ptz-speed__slider {
  padding: 0 4px;
}
.ptz-speed__slider :deep(.el-slider__runway) {
  margin: 8px 0;
}
/* Element Plus 滑块的 marks 文字色 */
.ptz-speed__slider :deep(.el-slider__marks-text) {
  font-size: 10px;
  color: var(--text-tertiary);
}
.ptz-speed__ticks {
  display: flex; justify-content: space-between;
  font-size: 11px;
  color: var(--text-tertiary);
  margin-top: -4px;
  padding: 0 4px;
}

.ptz-lens {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-top: 10px;
  border-top: 1px dashed var(--border-subtle);
}
.ptz-lens__group {
  display: grid;
  grid-template-columns: 56px 1fr;
  align-items: center;
  gap: 10px;
}
.ptz-lens__label {
  font-size: 12px;
  color: var(--text-secondary);
  font-weight: 500;
}
.ptz-lens__group :deep(.el-button-group) {
  width: 100%;
}
.ptz-lens__group :deep(.el-button-group .el-button) {
  flex: 1;
}
/* 镜头按钮按下期间高亮：与方向键保持一致的视觉反馈 */
.ptz-lens__group :deep(.el-button:active),
.ptz-lens__group :deep(.el-button.is-pressing) {
  background: rgba(11, 138, 178, 0.16);
  border-color: var(--brand-primary-500);
  color: var(--brand-primary-500);
}

/* ---- 文本与字体 ---- */
.mono { font-family: var(--font-mono); }
.text-tertiary { color: var(--text-tertiary); font-size: 12px; }

/* ---- 窄屏：上下堆叠 ---- */
@media (max-width: 900px) {
  .play-dialog { grid-template-columns: 1fr; }
}
</style>