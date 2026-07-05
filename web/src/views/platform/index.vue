<template>
  <div class="gb-page">
    <div class="gb-page__header">
      <div>
        <h1 class="gb-page__title">上级平台对接</h1>
        <p class="gb-page__subtitle">GB/T 28181 · 与上级 / 第三方视频平台的级联注册和共享</p>
      </div>
      <div class="gb-page__actions">
        <button class="gb-btn">心跳检测</button>
        <button class="gb-btn">导入</button>
        <button class="gb-btn gb-btn--primary">+ 新增平台</button>
      </div>
    </div>

    <section class="gb-grid gb-grid--kpi">
      <stat-card label="平台总数" :value="28" trend="已对接 26" trend-tone="success" :spark="[2,3,4,6,8,12,18,28]" />
      <stat-card label="注册成功" :value="26" value-tone="success" trend="成功率 92.8%" trend-tone="success" :spark="[60,68,72,80,86,90,92,92]" />
      <stat-card label="过期未续" :value="1" value-tone="warning" trend="1 个 6 时未续" trend-tone="neutral" :spark="[0,0,1,0,1,1,1,1]" />
      <stat-card label="拉流通道" :value="486" value-tone="primary" trend="来自 5 个平台" trend-tone="neutral" :spark="[100,150,200,300,400,420,450,486]" />
    </section>

    <article class="gb-card">
      <header class="gb-card-title">
        <span>平台列表</span>
        <div class="gb-toolbar">
          <div class="gb-search" style="flex:0 1 220px">
            <svg viewBox="0 0 24 24" fill="none"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.6"/><path d="M21 21l-4.35-4.35" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
            <input placeholder="搜索平台 / 域">
          </div>
        </div>
      </header>
      <el-table :data="rows" stripe size="small" style="width:100%">
        <el-table-column prop="name" label="平台名称" min-width="180" />
        <el-table-column prop="gbId" label="国标 ID" min-width="220">
          <template slot-scope="{ row }"><span class="mono text-tertiary">{{ row.gbId }}</span></template>
        </el-table-column>
        <el-table-column prop="sip" label="SIP 服务" min-width="220">
          <template slot-scope="{ row }"><span class="mono">{{ row.sip }}</span></template>
        </el-table-column>
        <el-table-column prop="type" label="对接方向" width="100">
          <template slot-scope="{ row }"><span :class="['gb-chip', 'gb-chip--' + row.typeTone]">{{ row.type }}</span></template>
        </el-table-column>
        <el-table-column prop="state" label="状态" width="100">
          <template slot-scope="{ row }"><span :class="['gb-chip', 'gb-chip--' + row.stateTone]">{{ row.state }}</span></template>
        </el-table-column>
        <el-table-column prop="channels" label="共享通道" width="100">
          <template slot-scope="{ row }"><span class="mono">{{ row.channels }}</span></template>
        </el-table-column>
        <el-table-column prop="expires" label="过期" min-width="140">
          <template slot-scope="{ row }"><span class="mono text-tertiary">{{ row.expires }}</span></template>
        </el-table-column>
        <el-table-column label="操作" align="right" width="200">
          <template slot-scope="{ row }">
            <button class="gb-btn-link">续期</button>
            <button class="gb-btn-link">通道</button>
            <button class="gb-btn-link">编辑</button>
          </template>
        </el-table-column>
      </el-table>
    </article>
  </div>
</template>

<script>
import StatCard from '@/components/StatCard'

export default {
  name: 'Platform',
  components: { StatCard },
  data() {
    return {
      rows: [
        { name: '广州市公安局交警支队', gbId: '13010000002000000001', sip: 'sip:10.20.4.5:5060', type: '上级', typeTone: 'primary', state: '已注册', stateTone: 'success', channels: 412, expires: '2030-12-31' },
        { name: '省厅视频专网', gbId: '13000000002000000001', sip: 'sip:10.30.4.5:5060', type: '上级', typeTone: 'primary', state: '已注册', stateTone: 'success', channels: 64, expires: '2030-12-31' },
        { name: '天河区城管平台', gbId: '44010000002000000001', sip: 'sip:10.40.4.5:5060', type: '平级', typeTone: 'info', state: '已注册', stateTone: 'success', channels: 8, expires: '2030-12-31' },
        { name: '海珠区应急平台', gbId: '41042200002000000001', sip: 'sip:10.50.4.5:5060', type: '平级', typeTone: 'info', state: '已注册', stateTone: 'success', channels: 12, expires: '2030-12-31' },
        { name: '黄埔区交通局', gbId: '51010000002000000001', sip: 'sip:10.60.4.5:5060', type: '下级', typeTone: 'success', state: '注册中', stateTone: 'warning', channels: 0, expires: '—' },
        { name: '广州公交集团', gbId: '44010000003000000001', sip: 'sip:10.70.4.5:5060', type: '下级', typeTone: 'success', state: '已注册', stateTone: 'success', channels: 482, expires: '2030-12-31' },
        { name: '南沙港务集团', gbId: '44010000004000000001', sip: 'sip:10.80.4.5:5060', type: '下级', typeTone: 'success', state: '过期', stateTone: 'error', channels: 16, expires: '2026-07-04' }
      ]
    }
  }
}
</script>
