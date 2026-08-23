<template>
  <div class="gb-page">
    <div class="gb-page__header">
      <div>
        <h1 class="gb-page__title">拉流代理 · 推流到第三方</h1>
        <p class="gb-page__subtitle">RTSP / RTMP / GB28181 · 将平台通道以推流方式转发至外部 CDN / 直播平台</p>
      </div>
      <div class="gb-page__actions">
        <button class="gb-btn">批量启停</button>
        <button class="gb-btn">导入</button>
        <button class="gb-btn gb-btn--primary">+ 新增代理</button>
      </div>
    </div>

    <section class="gb-grid gb-grid--kpi">
      <stat-card label="代理总数" :value="186" trend="RTSP ×98 · RTMP ×88" trend-tone="neutral" :spark="[10,12,15,18,22,28,32,38]" />
      <stat-card label="运行中" :value="172" value-tone="success" trend="运行率 92.4%" trend-tone="success" :spark="[60,68,72,80,86,90,92,92]" />
      <stat-card label="断流" :value="8" value-tone="error" trend="4 条拉流超时" trend-tone="neutral" :spark="[1,2,3,4,5,6,7,8]" />
      <stat-card label="总码率" value="42.8 Gbps" value-tone="warning" trend="峰值 16:20" trend-tone="neutral" :spark="[20,24,28,32,36,38,40,42]" />
    </section>

    <article class="gb-card">
      <header class="gb-card-title">
        <span>代理列表</span>
        <div class="gb-toolbar">
          <div class="gb-search" style="flex:0 1 220px">
            <svg viewBox="0 0 24 24" fill="none"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.6"/><path d="M21 21l-4.35-4.35" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
            <input v-model="kw" placeholder="搜索代理 / 通道">
          </div>
          <select v-model="f.type" class="search-mini">
            <option value="">全部协议</option>
            <option>RTSP</option>
            <option>RTMP</option>
            <option>GB28181</option>
          </select>
        </div>
      </header>
      <el-table :data="rows" stripe size="small" style="width:100%">
        <el-table-column prop="name" label="名称" min-width="160" />
        <el-table-column prop="channel" label="源通道" min-width="200">
          <template slot-scope="{ row }">
            <span class="mono text-tertiary">{{ row.channel }}</span>
            <div class="text-xs text-tertiary">{{ row.channelName }}</div>
          </template>
        </el-table-column>
        <el-table-column prop="type" label="协议" width="100">
          <template slot-scope="{ row }"><span :class="['gb-chip', 'gb-chip--' + row.typeTone]">{{ row.type }}</span></template>
        </el-table-column>
        <el-table-column prop="dest" label="目标地址" min-width="260">
          <template slot-scope="{ row }"><span class="mono">{{ row.dest }}</span></template>
        </el-table-column>
        <el-table-column prop="bitrate" label="码率" width="100">
          <template slot-scope="{ row }"><span class="mono">{{ row.bitrate }}</span></template>
        </el-table-column>
        <el-table-column prop="state" label="状态" width="100">
          <template slot-scope="{ row }">
            <el-switch v-model="row.on" :active-color="row.stateTone === 'success' ? '#16a34a' : '#0b8ab2'" />
            <span :class="['gb-chip', 'gb-chip--' + row.stateTone]" style="margin-left:6px">{{ row.state }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="updated" label="最近更新" width="120">
          <template slot-scope="{ row }"><span class="mono text-tertiary">{{ row.updated }}</span></template>
        </el-table-column>
        <el-table-column label="操作" align="right" width="200">
          <template slot-scope="{ row }">
            <button class="gb-btn-link">日志</button>
            <button class="gb-btn-link">编辑</button>
            <button class="gb-btn-link">复制</button>
          </template>
        </el-table-column>
      </el-table>
      <el-pagination layout="prev, pager, next, jumper, total" :total="186" :page-size="20" class="mt-2" />
    </article>
  </div>
</template>

<script>
import StatCard from '@/components/StatCard'

export default {
  name: 'StreamProxy',
  components: { StatCard },
  data() {
    return {
      kw: '',
      f: { type: '' },
      rows: [
        { name: '海珠门岗 · 抖音直播', channel: '41042200001320000102', channelName: '海珠门岗 · 东', type: 'RTMP', typeTone: 'info', dest: 'rtmp://push-douyin.com/live/abc123', bitrate: '4 Mbps', on: true, state: '运行中', stateTone: 'success', updated: '刚刚' },
        { name: '天河城 4F · 微信直播', channel: '44010000001310000003', channelName: '天河城 4F · 主', type: 'RTMP', typeTone: 'info', dest: 'rtmp://wx.tencent.com/live/xyz789', bitrate: '3 Mbps', on: true, state: '运行中', stateTone: 'success', updated: '12 秒前' },
        { name: '海珠仓库 1 · 阿里云 CDN', channel: '41042200001320000104', channelName: '海珠仓库 · 1', type: 'RTMP', typeTone: 'info', dest: 'rtmp://live.aliyun.com/live/cdn001', bitrate: '2 Mbps', on: true, state: '运行中', stateTone: 'success', updated: '30 秒前' },
        { name: '黄埔仓库 · 总部自建', channel: '51010000001310000008', channelName: '黄埔仓库 · A', type: 'RTSP', typeTone: 'primary', dest: 'rtsp://10.20.4.18:8554/live/main', bitrate: '6 Mbps', on: true, state: '运行中', stateTone: 'success', updated: '1 分前' },
        { name: '番禺园区 · 总部', channel: '51060000001310000001', channelName: '番禺园区 · 北', type: 'RTSP', typeTone: 'primary', dest: 'rtsp://10.20.4.20:8554/live/yard', bitrate: '4 Mbps', on: false, state: '已停止', stateTone: 'mute', updated: '8 时前' },
        { name: '公交 86 路 · 指挥中心', channel: 'JT-粤B·A8888', channelName: '公交 86 路 · A8888', type: 'GB28181', typeTone: 'success', dest: 'sip:13010000002000000001@10.20.4.5', bitrate: '2 Mbps', on: true, state: '断流', stateTone: 'error', updated: '2 分前' }
      ]
    }
  }
}
</script>

<style lang="scss" scoped>
.search-mini { padding: 4px 8px; font-size: 11px; border: 1px solid var(--border-default); background: var(--bg-elevated); border-radius: 4px; color: var(--text-primary); outline: 0; }
.search-mini:focus { border-color: var(--brand-primary-300); }
.mt-2 { margin-top: 8px; }
</style>
