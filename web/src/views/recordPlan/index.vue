<template>
  <div class="gb-page">
    <div class="gb-page__header">
      <div>
        <h1 class="gb-page__title">录像计划</h1>
        <p class="gb-page__subtitle">为通道设置定时/全时录像计划 · 支持全周、全日、按段</p>
      </div>
      <div class="gb-page__actions">
        <button class="gb-btn">模板库</button>
        <button class="gb-btn">批量分配</button>
        <button class="gb-btn gb-btn--primary">+ 新建计划</button>
      </div>
    </div>

    <section class="gb-grid gb-grid--kpi">
      <stat-card label="计划总数" :value="486" trend="全周 24×7 ×232" trend-tone="neutral" :spark="[2,4,6,8,12,15,18,22]" />
      <stat-card label="生效中" :value="432" value-tone="success" trend="88.9%" trend-tone="success" :spark="[60,68,72,80,86,90,92,88]" />
      <stat-card label="按需" :value="38" value-tone="warning" trend="触发式录像" trend-tone="neutral" :spark="[3,5,8,12,15,18,22,30]" />
      <stat-card label="冲突" :value="6" value-tone="error" trend="请前往 计划冲突 页处理" trend-tone="error" :spark="[1,1,2,3,4,5,6,6]" />
    </section>

    <article class="gb-card">
      <header class="gb-card-title">
        <span>计划列表</span>
        <div class="gb-toolbar">
          <div class="gb-search" style="flex:0 1 220px">
            <svg viewBox="0 0 24 24" fill="none"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.6"/><path d="M21 21l-4.35-4.35" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
            <input placeholder="搜索计划 / 通道">
          </div>
          <button class="gb-tab is-active">全周</button>
          <button class="gb-tab">按日</button>
          <button class="gb-tab">按段</button>
        </div>
      </header>
      <el-table :data="plans" stripe size="small" style="width:100%">
        <el-table-column prop="name" label="计划名称" min-width="200" />
        <el-table-column prop="type" label="类型" width="100">
          <template slot-scope="{ row }"><span :class="['gb-chip', 'gb-chip--' + row.typeTone]">{{ row.type }}</span></template>
        </el-table-column>
        <el-table-column prop="window" label="时间窗口" min-width="220">
          <template slot-scope="{ row }">
            <span class="mono">{{ row.start }} → {{ row.end }}</span>
            <div class="text-xs text-tertiary">{{ row.days }}</div>
          </template>
        </el-table-column>
        <el-table-column label="全周分布" min-width="220">
          <template slot-scope="{ row }">
            <div class="week-strip">
              <span v-for="(d, i) in ['一','二','三','四','五','六','日']" :key="i" :class="['week-cell', { 'on': row.daysMask[i] }]">{{ d }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="channels" label="通道数" width="100">
          <template slot-scope="{ row }"><span class="mono">{{ row.channels }}</span></template>
        </el-table-column>
        <el-table-column prop="state" label="状态" width="100">
          <template slot-scope="{ row }"><el-switch v-model="row.on" /></template>
        </el-table-column>
        <el-table-column prop="owner" label="创建人" min-width="120">
          <template slot-scope="{ row }">
            <span class="text-tertiary">{{ row.owner }}</span>
          </template>
        </el-table-column>
        <el-table-column label="操作" align="right" width="180">
          <template slot-scope="{ row }">
            <button class="gb-btn-link">详情</button>
            <button class="gb-btn-link">编辑</button>
            <button class="gb-btn-link">复制</button>
            <button class="gb-btn-link">删除</button>
          </template>
        </el-table-column>
      </el-table>
      <el-pagination layout="prev, pager, next, jumper, total" :total="486" :page-size="20" class="mt-2" />
    </article>
  </div>
</template>

<script>
import StatCard from '@/components/StatCard'

export default {
  name: 'RecordPlan',
  components: { StatCard },
  data() {
    return {
      plans: [
        { name: '海珠区 · 24×7 全周', type: '全周', typeTone: 'success', start: '00:00:00', end: '23:59:59', days: '周一 至 周日', daysMask: [1,1,1,1,1,1,1], channels: 412, on: true, owner: 'admin' },
        { name: '海珠仓库 · 工作时间', type: '按日', typeTone: 'info', start: '08:00:00', end: '20:00:00', days: '周一 至 周五', daysMask: [1,1,1,1,1,0,0], channels: 64, on: true, owner: 'admin' },
        { name: '海珠仓库 · 夜间移动侦测', type: '按段', typeTone: 'warning', start: '20:00:00', end: '08:00:00', days: '全周 + 移动侦测', daysMask: [1,1,1,1,1,1,1], channels: 32, on: true, owner: 'admin' },
        { name: '天河城商圈 · 全天', type: '全周', typeTone: 'success', start: '00:00:00', end: '23:59:59', days: '全周', daysMask: [1,1,1,1,1,1,1], channels: 98, on: true, owner: 'ops-tianhe' },
        { name: '高速 K127 · 重点段', type: '按段', typeTone: 'warning', start: '06:00:00', end: '22:00:00', days: '全周', daysMask: [1,1,1,1,1,1,1], channels: 12, on: true, owner: 'admin' },
        { name: '停车场 B2 · 周末加时', type: '按日', typeTone: 'info', start: '00:00:00', end: '23:59:59', days: '周六、周日', daysMask: [0,0,0,0,0,1,1], channels: 18, on: true, owner: 'admin' },
        { name: '危险品车辆 · 触发录像', type: '按需', typeTone: 'error', start: '—', end: '—', days: '触发后 5 分钟', daysMask: [1,1,1,1,1,1,1], channels: 217, on: true, owner: 'admin' }
      ]
    }
  }
}
</script>

<style lang="scss" scoped>
.week-strip { display: grid; grid-template-columns: repeat(7, 1fr); gap: 2px; }
.week-cell { text-align: center; font-size: 10px; padding: 3px 0; border-radius: 3px; background: var(--bg-elevated); color: var(--text-tertiary); }
.week-cell.on { background: var(--brand-primary-500); color: #fff; }
.mt-2 { margin-top: 8px; }
</style>
