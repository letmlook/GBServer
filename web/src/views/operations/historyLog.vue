<template>
  <div class="gb-page">
    <div class="gb-page__header">
      <div>
        <h1 class="gb-page__title">系统日志</h1>
        <p class="gb-page__subtitle">历史日志检索 · 8,261,492 条 · 保留 90 天</p>
      </div>
      <div class="gb-page__actions">
        <div class="gb-search" style="flex:0 1 220px">
          <svg viewBox="0 0 24 24" fill="none"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.6"/><path d="M21 21l-4.35-4.35" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
          <input placeholder="搜索关键字 / 模块">
        </div>
        <button class="gb-btn">导出 CSV</button>
      </div>
    </div>

    <section class="gb-grid gb-grid--kpi">
      <stat-card label="今日日志" :value="118326" value-tone="default" trend="INFO 84.2% · WARN 11.6% · ERROR 4.2%" trend-tone="neutral" :spark="[10,12,18,22,28,32,40,48]" />
      <stat-card label="错误日志" :value="4986" value-tone="error" trend="较昨日 ↓ 2.4%" trend-tone="success" :spark="[18,16,14,12,10,8,8,6]" />
      <stat-card label="异常率" value="4.2%" value-tone="warning" trend="基线 5% · 在范围内" trend-tone="neutral" :spark="[5,5,4,4,5,4,4,4]" />
      <stat-card label="存储占用" value="2.4 TB" value-tone="primary" trend="归档 90 天前到 OSS" trend-tone="neutral" :spark="[1.8,1.9,2,2.1,2.2,2.3,2.4,2.4]" />
    </section>

    <article class="gb-card">
      <header class="gb-card-title">
        <span>日志条目</span>
        <div class="gb-toolbar">
          <select class="search-mini">
            <option>全部级别</option><option>INFO</option><option>WARN</option><option>ERROR</option>
          </select>
          <select class="search-mini">
            <option>全部模块</option><option>SIP</option><option>ZLM</option><option>DB</option><option>JT</option>
          </select>
          <el-date-picker v-model="range" type="daterange" size="small" style="width:240px" range-separator="→" start-placeholder="起始" end-placeholder="结束" />
        </div>
      </header>
      <el-table :data="rows" stripe size="small" style="width:100%">
        <el-table-column prop="time" label="时间" width="160">
          <template slot-scope="{ row }"><span class="mono text-tertiary">{{ row.time }}</span></template>
        </el-table-column>
        <el-table-column prop="level" label="级别" width="80">
          <template slot-scope="{ row }"><span :class="['gb-chip', 'gb-chip--' + row.tone]">{{ row.level }}</span></template>
        </el-table-column>
        <el-table-column prop="module" label="模块" width="120">
          <template slot-scope="{ row }"><span class="text-tertiary">{{ row.module }}</span></template>
        </el-table-column>
        <el-table-column prop="actor" label="操作人" min-width="160">
          <template slot-scope="{ row }"><span class="mono">{{ row.actor }}</span></template>
        </el-table-column>
        <el-table-column prop="event" label="事件" min-width="280" />
        <el-table-column prop="ip" label="来源 IP" width="120">
          <template slot-scope="{ row }"><span class="mono text-tertiary">{{ row.ip }}</span></template>
        </el-table-column>
        <el-table-column label="操作" align="right" width="120">
          <template slot-scope="{ row }"><button class="gb-btn-link">详情</button></template>
        </el-table-column>
      </el-table>
      <el-pagination layout="prev, pager, next, jumper, total" :total="8261492" :page-size="20" class="mt-2" />
    </article>
  </div>
</template>

<script>
import StatCard from '@/components/StatCard'

export default {
  name: 'HistoryLog',
  components: { StatCard },
  data() {
    return {
      range: [],
      rows: [
        { time: '2026-07-05 16:42:18', level: 'ERROR', tone: 'error', module: 'SIP', actor: 'system', event: '收到 41042200001320000102 的 BYE，超时重试失败', ip: '10.21.4.118' },
        { time: '2026-07-05 16:41:50', level: 'WARN', tone: 'warning', module: 'SIP', actor: 'system', event: 'sip-gw-beijing 注册超时（>3s）', ip: '10.30.4.5' },
        { time: '2026-07-05 16:38:09', level: 'INFO', tone: 'info', module: 'DB', actor: 'admin', event: '新建录像计划：海珠区 · 24×7 全周', ip: '127.0.0.1' },
        { time: '2026-07-05 16:32:44', level: 'ERROR', tone: 'error', module: 'JT', actor: 'system', event: 'JT-粤B·A8888 GPS 信号丢失，重试 3 次后告警', ip: '—' },
        { time: '2026-07-05 16:28:21', level: 'INFO', tone: 'info', module: 'ZLM', actor: 'system', event: '存储节点切换：edge-04 → edge-02', ip: '10.21.4.22' },
        { time: '2026-07-05 16:24:02', level: 'INFO', tone: 'info', module: 'Auth', actor: 'ops-tianhe', event: '登录成功，IP 10.20.4.21', ip: '10.20.4.21' },
        { time: '2026-07-05 16:20:33', level: 'WARN', tone: 'warning', module: 'Storage', actor: 'system', event: '录像 2026-07-04-15.zip 写入失败，已重试', ip: '10.20.4.18' },
        { time: '2026-07-05 16:18:11', level: 'INFO', tone: 'info', module: 'Cascade', actor: 'system', event: '已成功注册到上级：广州市公安局交警支队', ip: '10.20.4.5' }
      ]
    }
  }
}
</script>

<style lang="scss" scoped>
.search-mini { padding: 4px 8px; font-size: 11px; border: 1px solid var(--border-default); background: var(--bg-elevated); border-radius: 4px; color: var(--text-primary); outline: 0; }
.mt-2 { margin-top: 8px; }
</style>
