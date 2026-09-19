import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'

export interface LogRecord {
  id?: number
  time?: string
  level?: string
  logger?: string
  thread?: string
  message?: string
  source?: string
  tone?: 'info' | 'warn' | 'error' | 'debug'
}

export interface LogQueryParams {
  page?: number
  count?: number
  query?: string
  startTime?: string
  endTime?: string
  level?: string
}

export function getLogList(params: LogQueryParams) {
  return request<WvpResult<{ total: number; list: LogRecord[] }>>({
    method: 'get',
    url: '/log/list',
    params
  })
}

export function getLogFile(fileName: string) {
  return request<Blob>({
    method: 'get',
    url: `/log/file/${encodeURIComponent(fileName)}`,
    responseType: 'blob'
  })
}

/**
 * `GET /api/server/system/info`。
 *
 * 注意 `cpu` / `mem` / `net` 是**折线图的历史采样数组**（dashboard 的 v-charts
 * 直接 setData 覆盖渲染），当前的使用率是标量 `cpu_usage` / `mem_usage` /
 * `disk_usage`（0-100 的百分数）。
 * 此前类型把 `cpu` 声明成 `number`，系统信息页按 number 渲染 →
 * 卡片显示成 "[object Object]"，进度条 percentage 收到数组而失效。
 */
export interface SystemInfo {
  /** 历史采样：data 是 0-1 的小数 */
  cpu?: { time: string; data: number }[]
  mem?: { time: string; data: number }[]
  net?: { time: string; out: number; in: number }[]
  /** load average 1min 时间序列（5m/15m 取当前值画水平线） */
  load?: { time: string; data: number }[]
  /** 当前 load average 1/5/15min 标量（来自 /proc/loadavg，Linux only） */
  load_avg?: { '1'?: number; '5'?: number; '15'?: number } | null
  cpu_usage?: number
  mem_usage?: number
  disk_usage?: number
  /** 磁盘使用率 ring buffer：每个点 = 当时所有挂载点中最大 used% */
  disk_history?: { time: string; data: number }[]
  memory?: {
    /** 字节 */
    total?: number
    used?: number
    free?: number
    mem?: { data: number; time: string }[]
  }
  /** 每个挂载点：`total`/`used` 是字节（系统信息页卡片），`use`/`free` 是 GB（dashboard 柱状图） */
  disk?: { path: string; total: number; used: number; use?: number; free?: number }[]
  /** 不过滤的全量挂载点（含 tmpfs / loop / overlay）。dashboard "全部磁盘"柱状图用 */
  disk_all?: { fs: string; path: string; total: number; used: number; use: number; free: number }[]
  network?: { name: string; rx: number; tx: number }[]
  netTotal?: number
  uptime?: number
  version?: string
  buildTime?: string
  mediaServerCount?: number
  deviceOnline?: number
  deviceTotal?: number
  channelOnline?: number
  channelTotal?: number
  /** SIP / GB28181 平台配置（密码明文，admin 用于把设备/下级平台对接进来） */
  sip_config?: {
    enabled?: boolean
    ip?: string
    bind_ip?: string | null
    port?: number
    tcp_port?: number
    tcp_enabled?: boolean
    device_id?: string
    /** SIP 用户名：级联登录外部 SIP 网络时使用的账号；空时降级为 device_id */
    username?: string
    realm?: string
    /** 明文：要给设备/下级平台填入 */
    password?: string
    keepalive_timeout?: number
    register_timeout?: number
    charset?: string
    sdp_ip?: string | null
    stream_ip?: string | null
  } | null
  /** JT1078 车载终端协议配置 */
  jt1078_config?: {
    tcp_port?: number
    udp_port?: number
    timeout_ms?: number | null
    retransmit_wait_ms?: number | null
    retransmit_hook_url?: string | null
  } | null
  /** 本机对外 IP（自动探测，OS 选出的可路由 NIC IP）。
   *  用来给 admin 填到设备/下级平台的「服务器地址」里。 */
  host_ip?: string | null
}

export function getSystemInfo() {
  return request<WvpResult<SystemInfo>>({
    method: 'get',
    url: '/server/system/info'
  })
}

export function getSystemConfigInfo() {
  return request<WvpResult<Record<string, unknown>>>({
    method: 'get',
    url: '/server/system/configInfo'
  })
}

export function getResourceInfo() {
  return request<WvpResult<Record<string, unknown>>>({
    method: 'get',
    url: '/server/resource/info'
  })
}

export function getServerInfo() {
  return request<WvpResult<Record<string, unknown>>>({
    method: 'get',
    url: '/server/info'
  })
}
