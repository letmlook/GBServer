<template>
  <div class="gb-page channel-page">
    <div class="gb-page__header">
      <div>
        <h1 class="gb-page__title">通道列表</h1>
        <p class="gb-page__subtitle">3,841 个通道 · 来自 412 台设备 · GB/T 28181</p>
      </div>
      <div class="gb-page__actions">
        <button class="gb-btn">批量操作</button>
        <button class="gb-btn">导出</button>
        <button class="gb-btn gb-btn--primary">+ 新增通道</button>
      </div>
    </div>

    <!-- KPI 统计 -->
    <section class="gb-grid gb-grid--kpi">
      <stat-card label="通道总数" :value="3841" trend="↑ 4.2% 较昨日" trend-tone="success" :spark="[12,15,18,20,22,28,32,38]" />
      <stat-card label="在线通道" :value="2915" value-tone="success" trend="在线率 75.9%" trend-tone="neutral" :spark="[20,24,22,28,32,30,36,40]" />
      <stat-card label="录制中" :value="1247" value-tone="warning" trend="占总数 32.5%" trend-tone="neutral" :spark="[10,12,14,18,16,18,22,24]" />
      <stat-card label="故障" :value="23" value-tone="error" trend="严重 3 · 离线 20" trend-tone="neutral" :spark="[3,5,2,4,6,4,5,3]" />
    </section>

    <!-- 筛选条 -->
    <section class="gb-card">
      <div class="gb-filterbar">
        <span class="gb-filterbar__label">设备</span>
        <el-select v-model="f.device" size="small" placeholder="全部设备" clearable style="width:180px">
          <el-option label="全部设备" value="" />
          <el-option label="44010000001310000001" value="1" />
        </el-select>
        <span class="gb-filterbar__label">状态</span>
        <el-select v-model="f.state" size="small" placeholder="全部状态" clearable style="width:120px">
          <el-option label="在线" value="online" />
          <el-option label="离线" value="offline" />
          <el-option label="录制中" value="rec" />
        </el-select>
        <span class="gb-filterbar__label">关键字</span>
        <el-input v-model="f.kw" size="small" placeholder="国标 ID / 名称" style="width:220px" />
        <div class="gb-filterbar__right">
          <button class="gb-btn">重置</button>
          <button class="gb-btn gb-btn--primary">查询</button>
        </div>
      </div>
    </section>

    <!-- 表格 -->
    <article class="gb-card">
      <header class="gb-card-title">
        <span>通道列表</span>
        <div class="gb-toolbar">
          <span class="text-tertiary text-xs">已选 0 项</span>
          <button class="gb-btn">开启</button>
          <button class="gb-btn">关闭</button>
          <button class="gb-btn">批量录像</button>
          <button class="gb-btn gb-btn--danger">删除</button>
        </div>
      </header>
      <el-table :data="rows" stripe size="small" @selection-change="sel = $event" style="width:100%">
        <el-table-column type="selection" width="40" />
        <el-table-column prop="id" label="国标 ID" min-width="200">
          <template slot-scope="{ row }"><span class="mono text-primary-accent">{{ row.id }}</span></template>
        </el-table-column>
        <el-table-column prop="name" label="通道名称" min-width="180" />
        <el-table-column prop="device" label="父设备" min-width="160">
          <template slot-scope="{ row }"><span class="mono text-tertiary">{{ row.device }}</span></template>
        </el-table-column>
        <el-table-column prop="state" label="状态" width="100">
          <template slot-scope="{ row }"><span :class="['gb-chip', 'gb-chip--' + row.stateTone]">{{ row.state }}</span></template>
        </el-table-column>
        <el-table-column prop="record" label="录像" width="100">
          <template slot-scope="{ row }"><el-switch v-model="row.recordOn" /></template>
        </el-table-column>
        <el-table-column prop="resolution" label="分辨率" width="100">
          <template slot-scope="{ row }"><span class="mono">{{ row.resolution }}</span></template>
        </el-table-column>
        <el-table-column prop="region" label="所属区域" min-width="140">
          <template slot-scope="{ row }"><span class="text-tertiary">{{ row.region }}</span></template>
        </el-table-column>
        <el-table-column prop="latency" label="延迟" width="80">
          <template slot-scope="{ row }"><span class="mono">{{ row.latency }}</span></template>
        </el-table-column>
        <el-table-column prop="updated" label="最后心跳" width="120">
          <template slot-scope="{ row }"><span class="mono text-tertiary">{{ row.updated }}</span></template>
        </el-table-column>
        <el-table-column label="操作" align="right" width="200">
          <template slot-scope="{ row }">
            <button class="gb-btn-link" @click="$router.push('/playback')">回放</button>
            <button class="gb-btn-link" @click="$router.push('/live')">预览</button>
            <button class="gb-btn-link">编辑</button>
            <button class="gb-btn-link">删除</button>
          </template>
        </el-table-column>
      </el-table>
      <el-pagination layout="prev, pager, next, jumper, total" :total="200" :page-size="10" class="mt-2" />
    </article>
  </div>
</template>

<script>
import StatCard from '@/components/StatCard'

export default {
  name: 'ChannelList',
  components: { StatCard },
  data() {
    return {
      f: { device: '', state: '', kw: '' },
      sel: [],
      rows: [
        { id: '41042200001320000102', name: '海珠门岗 · 东', device: '44010000001310000001', state: '告警', stateTone: 'error', recordOn: true, resolution: '4K', region: '海珠区', latency: '12ms', updated: '刚刚' },
        { id: '41042200001320000103', name: '海珠门岗 · 西', device: '44010000001310000001', state: '在线', stateTone: 'success', recordOn: true, resolution: '4K', region: '海珠区', latency: '14ms', updated: '5 秒前' },
        { id: '41042200001320000104', name: '海珠仓库 · 1', device: '44010000001310000002', state: '在线', stateTone: 'success', recordOn: false, resolution: '1080P', region: '海珠区', latency: '18ms', updated: '1 分前' },
        { id: '41042200001320000105', name: '海珠仓库 · 2', device: '44010000001310000002', state: '在线', stateTone: 'success', recordOn: true, resolution: '1080P', region: '海珠区', latency: '15ms', updated: '2 分前' },
        { id: '44010000001310000003', name: '天河城 4F · 主', device: '44010000001310000001', state: '在线', stateTone: 'success', recordOn: true, resolution: '1080P', region: '天河区', latency: '9ms', updated: '8 秒前' },
        { id: '44010000001310000006', name: '天河城 3F · 西', device: '44010000001310000007', state: '离线', stateTone: 'warning', recordOn: false, resolution: '1080P', region: '天河区', latency: '—', updated: '2 时前' },
        { id: '51010000001310000008', name: '黄埔仓库 · A', device: '51010000001310000001', state: '在线', stateTone: 'success', recordOn: true, resolution: '4K', region: '黄埔区', latency: '22ms', updated: '30 秒前' },
        { id: '51010000001310000009', name: '黄埔园区 · 西门', device: '51010000001310000002', state: '在线', stateTone: 'success', recordOn: true, resolution: '1080P', region: '黄埔区', latency: '20ms', updated: '1 分前' },
        { id: '51060000001310000001', name: '番禺园区 · 北', device: '51060000001310000001', state: '离线', stateTone: 'warning', recordOn: false, resolution: '720P', region: '番禺区', latency: '—', updated: '8 时前' },
        { id: '51060000001310000002', name: '番禺大桥', device: '51060000001310000001', state: '在线', stateTone: 'success', recordOn: true, resolution: '4K', region: '番禺区', latency: '18ms', updated: '15 秒前' }
      ]
    }
  }
}
</script>

<style lang="scss" scoped>
.channel-page { gap: 14px; padding-top: 16px; }
.mt-2 { margin-top: 8px; }
</style>
