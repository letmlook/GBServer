<template>
  <span class="talk-panel">
    <el-button
      size="small"
      :type="active ? 'danger' : 'primary'"
      :plain="!active"
      :loading="busy"
      :disabled="!deviceId || !channelId"
      @click="toggle"
    >
      {{ active ? '结束对讲' : '对讲' }}
    </el-button>
    <span v-if="active" class="talk-panel__hint">
      上行 {{ stats.packets }} 包 / {{ stats.bytes }} 字节
      <template v-if="stats.error"> · {{ stats.error }}</template>
    </span>
  </span>
</template>

<script setup lang="ts">
/**
 * 语音对讲面板：麦克风 → 8kHz PCM → WebSocket → 服务端 G.711A/RTP → 设备。
 *
 * 说明：
 * - 上行（浏览器 → 设备）走本组件建立的 WebSocket；
 * - 下行（设备 → 浏览器）由设备按对讲 SDP 把 RTP 推到 ZLM 的 RTP server
 *   （`TalkSession.localPort`），浏览器播放 ZLM 输出的 ws-flv 即可，
 *   不经过本组件的 WebSocket。
 */
import { computed, onBeforeUnmount, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { startTalk, stopTalk, talkAudioWsUrl } from '@/api/talk'

const props = defineProps<{ deviceId?: string; channelId?: string }>()

/** 目标采样率：G.711 / PCMA 固定 8kHz */
const TARGET_RATE = 8000
/** 每次通过 WebSocket 发送的样本数（与后端 20ms/160 样本的帧长对齐：2 帧） */
const CHUNK_SAMPLES = 320

const active = ref(false)
const busy = ref(false)
const stats = reactive({ packets: 0, bytes: 0, error: '' })

let ws: WebSocket | null = null
let stream: MediaStream | null = null
let audioCtx: AudioContext | null = null
let source: MediaStreamAudioSourceNode | null = null
let processor: ScriptProcessorNode | null = null
/** 多相重采样：输入流上的小数游标 + 余量缓冲 */
let resamplePos = 0
let pending = new Int16Array(0)

const deviceId = computed(() => props.deviceId ?? '')
const channelId = computed(() => props.channelId ?? '')

/** Float32 [-1,1] → i16，带削波保护 */
function floatToPcm16(input: Float32Array): Int16Array {
  const out = new Int16Array(input.length)
  for (let i = 0; i < input.length; i++) {
    const s = Math.max(-1, Math.min(1, input[i]))
    out[i] = s < 0 ? s * 0x8000 : s * 0x7fff
  }
  return out
}

/**
 * 把任意采样率的 PCM 线性重采样到 8kHz。
 *
 * 用小数游标（而不是"每 ratio 个取一个"）以便正确处理 44100 → 8000 这种
 * 非整数比；剩余不足一个输出点的部分留到下一次调用。
 */
function resampleTo8k(input: Int16Array, inputRate: number): Int16Array {
  if (inputRate === TARGET_RATE) return input
  const ratio = inputRate / TARGET_RATE
  const out: number[] = []
  let pos = resamplePos
  while (pos + 1 < input.length) {
    const i0 = Math.floor(pos)
    const frac = pos - i0
    const v = input[i0] * (1 - frac) + input[i0 + 1] * frac
    out.push(Math.round(v))
    pos += ratio
  }
  resamplePos = pos - input.length
  if (resamplePos < 0) resamplePos = 0
  return Int16Array.from(out)
}

function appendPending(chunk: Int16Array) {
  const merged = new Int16Array(pending.length + chunk.length)
  merged.set(pending, 0)
  merged.set(chunk, pending.length)
  pending = merged
}

function flushPending() {
  while (pending.length >= CHUNK_SAMPLES && ws && ws.readyState === WebSocket.OPEN) {
    const slice = pending.subarray(0, CHUNK_SAMPLES)
    // 复制一份：subarray 是视图，发送后 pending 还会被复用
    const payload = new Int16Array(slice)
    ws.send(payload.buffer)
    stats.packets += 1
    stats.bytes += payload.byteLength
    pending = pending.subarray(CHUNK_SAMPLES)
  }
  // 把视图转成独立数组，避免持有整块内存
  if (pending.length && pending.byteOffset !== 0) {
    pending = new Int16Array(pending)
  }
}

async function start() {
  if (!deviceId.value || !channelId.value) return
  busy.value = true
  stats.error = ''
  try {
    // 1) 后端发 SIP INVITE 并等设备 200 OK（返回 status=active + 设备音频地址）；
    //    设备不应答时这里会抛出明确的错误信息
    const talk = await startTalk(deviceId.value, channelId.value)
    const talkData = talk.data
    if (!talkData || talkData.status !== 'active') {
      throw new Error(`对讲未建立（status=${talkData?.status ?? 'unknown'}）`)
    }

    // 2) 建立上行音频 WebSocket
    const url = talkAudioWsUrl(deviceId.value, channelId.value)
    ws = new WebSocket(url)
    ws.binaryType = 'arraybuffer'
    await new Promise<void>((resolve, reject) => {
      const timer = window.setTimeout(() => reject(new Error('WebSocket 连接超时')), 8000)
      ws!.onopen = () => {
        window.clearTimeout(timer)
        resolve()
      }
      ws!.onerror = () => {
        window.clearTimeout(timer)
        reject(new Error('WebSocket 连接失败（音频通道未建立）'))
      }
    })
    ws.onmessage = (ev) => {
      if (typeof ev.data === 'string') {
        try {
          const msg = JSON.parse(ev.data) as { error?: string; packets?: number; bytes?: number }
          if (msg.error) stats.error = msg.error
          if (typeof msg.packets === 'number') stats.packets = msg.packets
          if (typeof msg.bytes === 'number') stats.bytes = msg.bytes
        } catch {
          /* 非 JSON 文本忽略 */
        }
      }
    }

    // 3) 采集麦克风
    stream = await navigator.mediaDevices.getUserMedia({
      audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true }
    })
    audioCtx = new AudioContext()
    source = audioCtx.createMediaStreamSource(stream)
    // ScriptProcessorNode 虽已标记废弃，但兼容性最好且无需额外静态资源
    // （AudioWorklet 需要单独的文件 + addModule，收益仅是省一点主线程开销）。
    processor = audioCtx.createScriptProcessor(2048, 1, 1)
    resamplePos = 0
    pending = new Int16Array(0)
    processor.onaudioprocess = (ev) => {
      if (!ws || ws.readyState !== WebSocket.OPEN) return
      const input = ev.inputBuffer.getChannelData(0)
      const pcm = floatToPcm16(input)
      const pcm8k = resampleTo8k(pcm, audioCtx?.sampleRate ?? TARGET_RATE)
      if (!pcm8k.length) return
      appendPending(pcm8k)
      flushPending()
    }
    source.connect(processor)
    // 某些浏览器只有在节点连到 destination 时才会驱动 onaudioprocess；
    // 用一个 0 增益节点避免把自己的声音回放出来。
    const mute = audioCtx.createGain()
    mute.gain.value = 0
    processor.connect(mute)
    mute.connect(audioCtx.destination)

    active.value = true
    ElMessage.success('对讲已开始，请对着麦克风说话')
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    stats.error = msg
    ElMessage.error(`开启对讲失败：${msg}`)
    await teardown(false)
    // 会话已经建立但本地采集失败时，顺手结束掉后端会话，避免悬挂
    stopTalk(deviceId.value, channelId.value).catch(() => {})
  } finally {
    busy.value = false
  }
}

async function teardown(sendBye: boolean) {
  try {
    processor?.disconnect()
    source?.disconnect()
  } catch {
    /* 忽略断开异常 */
  }
  processor = null
  source = null
  stream?.getTracks().forEach((t) => t.stop())
  stream = null
  if (audioCtx && audioCtx.state !== 'closed') {
    await audioCtx.close().catch(() => {})
  }
  audioCtx = null
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.close(1000, 'talk stopped')
  }
  ws = null
  pending = new Int16Array(0)
  resamplePos = 0
  const wasActive = active.value
  active.value = false
  if (sendBye && wasActive && deviceId.value && channelId.value) {
    await stopTalk(deviceId.value, channelId.value).catch((e) => {
      ElMessage.warning(`结束对讲信令失败：${e instanceof Error ? e.message : String(e)}`)
    })
  }
}

async function toggle() {
  if (active.value) {
    busy.value = true
    try {
      await teardown(true)
      ElMessage.info('对讲已结束')
    } finally {
      busy.value = false
    }
  } else {
    await start()
  }
}

onBeforeUnmount(() => {
  // 组件卸载时静默收尾，避免麦克风一直开着
  void teardown(true)
})
</script>

<style scoped>
.talk-panel {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.talk-panel__hint {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
</style>
