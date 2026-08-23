/**
 * GB/T 28181 业务模型 VO 集中定义
 * 替代散落在各 api/*.ts 里的重复 interface，避免循环依赖
 */

// ============= 设备 / 通道 =============

export interface DeviceVO {
  id?: number
  deviceId: string
  name?: string
  manufacturer?: string
  model?: string
  firmware?: string
  transport?: 'UDP' | 'TCP'
  streamMode?: 'UDP' | 'TCP' | 'TCP-ACTIVE' | 'TCP-PASSIVE'
  ip?: string
  port?: number
  expires?: number
  heartBeatInterval?: number
  heartBeatCount?: number
  registerTime?: string
  updateTime?: string
  createTime?: string
  online?: number | boolean
  channelCount?: number
  mediaServerId?: string
  sdpIp?: string
  status?: 'ON' | 'OFF' | string
  gbId?: string
  gbDeviceId?: string
  treePath?: string
  password?: string
}

export interface ChannelVO {
  id?: number
  channelId: string
  deviceId?: string
  name?: string
  manufacturer?: string
  model?: string
  owner?: string
  civilCode?: string
  address?: string
  status?: 'ON' | 'OFF' | string
  parental?: string
  parentId?: string
  longitude?: number
  latitude?: number
  streamIdentification?: string
  channelType?: number
  hasAudio?: boolean
  audio?: boolean
  subCount?: number
  registerStatus?: string
  createTime?: string
  updateTime?: string
}

// ============= 平台 / 用户 =============

export interface PlatformVO {
  id?: number
  serverGbId: string
  serverIp?: string
  serverPort?: number
  name?: string
  username?: string
  password?: string
  realm?: string
  transport?: 'UDP' | 'TCP'
  registerInterval?: number
  heartBeatInterval?: number
  heartBeatCount?: number
  expires?: number
  enable?: boolean
  status?: boolean
  createTime?: string
  updateTime?: string
}

export interface UserVO {
  id?: number
  username: string
  password?: string
  roleId?: number
  roleName?: string
  pushKey?: string
  createTime?: string
  updateTime?: string
}

// ============= 媒体 / 录像 / 计划 =============

export interface MediaServerVO {
  id?: string
  ip: string
  httpPort: number
  rtspPort?: number
  rtmpPort?: number
  secret: string
  enabled?: boolean
  hookAliveInterval?: number
  status?: boolean
  type?: string
  streamMode?: string
  createTime?: string
  updateTime?: string
  lastKeepaliveTime?: string
  lastRegisterTime?: string
}

export interface CloudRecordVO {
  id?: number
  app?: string
  stream?: string
  callId?: string
  mediaServerId?: string
  gbId?: string
  startTime?: string
  endTime?: string
  filePath?: string
  folder?: string
  size?: number
  createTime?: string
}

export interface RecordPlanVO {
  id?: number
  name?: string
  planType?: string
  startTime?: string
  endTime?: string
  enable?: boolean
  enableTime?: boolean
  mon?: boolean
  tue?: boolean
  wed?: boolean
  thu?: boolean
  fri?: boolean
  sat?: boolean
  sun?: boolean
  channelCount?: number
  createTime?: string
  updateTime?: string
}

// ============= 流 / 报警 =============

export interface StreamProxyVO {
  id?: number
  name: string
  type?: 'rtsp' | 'rtmp' | 'hls'
  app?: string
  stream?: string
  url?: string
  destUrl?: string
  enabled?: boolean
  status?: number
  createTime?: string
  updateTime?: string
}

export interface StreamPushVO {
  id?: number
  app: string
  stream: string
  gbId?: string
  status?: number
  url?: string
  mediaServerId?: string
  createTime?: string
  updateTime?: string
}

export interface AlarmVO {
  id?: number
  deviceId?: string
  channelId?: string
  alarmLevel?: string
  alarmMethod?: string
  alarmType?: string
  alarmTime?: string
  alarmDescription?: string
  longitude?: number
  latitude?: number
  handled?: boolean
  handleTime?: string
  handleUser?: string
  handleResult?: string
  snapUrl?: string
  videoUrl?: string
}

// ============= JT/T 1078 =============

export interface JtTerminalVO {
  id?: number
  phoneNumber: string
  terminalId?: string
  provinceId?: string
  provinceText?: string
  cityId?: string
  cityText?: string
  makerId?: string
  model?: string
  plateColor?: number
  plateNo?: string
  longitude?: number
  latitude?: number
  status?: number
  mediaServerId?: string
  sdpIp?: string
  authCode?: string
  registerTime?: string
  updateTime?: string
  createTime?: string
}

export interface JtAreaVO {
  id?: number
  phoneNumber: string
  label?: string
  shape?: 'circle' | 'polygon' | 'rectangle'
  centerLat?: number
  centerLon?: number
  radiusM?: number
  ltLat?: number
  ltLon?: number
  rbLat?: number
  rbLon?: number
  pointsJson?: string
  createTime?: string
  updateTime?: string
}

export interface JtRouteVO {
  id?: number
  phoneNumber: string
  label?: string
  waypointsJson?: string
  createTime?: string
  updateTime?: string
}
