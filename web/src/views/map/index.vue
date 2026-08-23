<template>
  <div class="map-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">电子地图</h1>
        <p class="page-subtitle">WGS84 / GCJ02 坐标系 · 通道定位</p>
      </div>
      <div class="page-actions">
        <el-radio-group v-model="coord" size="small">
          <el-radio-button label="WGS84">WGS84</el-radio-button>
          <el-radio-button label="GCJ02">GCJ02</el-radio-button>
        </el-radio-group>
      </div>
    </div>

    <el-row :gutter="12">
      <el-col :xs="24" :md="6">
        <el-card class="region-card">
          <template #header><span>行政区划</span></template>
          <el-tree
            :data="regions"
            node-key="id"
            :props="{ label: 'name', children: 'children' }"
            :default-expand-all="true"
            highlight-current
            @node-click="onRegionClick"
          />
        </el-card>
      </el-col>
      <el-col :xs="24" :md="18">
        <el-card class="map-card">
          <div class="map-stage">
            <svg viewBox="0 0 800 500" class="map-svg" preserveAspectRatio="xMidYMid meet">
              <!-- 简化地图背景 -->
              <defs>
                <pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse">
                  <path d="M 40 0 L 0 0 0 40" fill="none" stroke="rgba(11,138,178,0.10)" />
                </pattern>
              </defs>
              <rect width="800" height="500" fill="url(#grid)" />

              <!-- 通道点位 -->
              <g v-for="(p, i) in points" :key="i" :transform="`translate(${p.x}, ${p.y})`" class="map-point" @click="onPointClick(p)">
                <circle r="8" :fill="p.online ? '#16a34a' : '#94a3b8'" stroke="#fff" stroke-width="2" />
                <title>{{ p.name }} ({{ p.channelId }})</title>
              </g>
            </svg>
            <div class="map-legend">
              <span><span class="dot dot-on" />在线</span>
              <span><span class="dot dot-off" />离线</span>
              <span>坐标: {{ coord }}</span>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { getRegionTreeList, type Region } from '@/api/region'

const regions = ref<Region[]>([])
const coord = ref<'WGS84' | 'GCJ02'>('WGS84')
const points = ref<{ x: number; y: number; name: string; channelId: string; online: boolean }[]>([])

function onRegionClick(node: Region) {
  // 简化：根据 regionId 生成示例点位；真实场景应调 /api/common/channel/list 并筛选
  const cx = 400 + Math.cos(node.id ?? 0) * 200
  const cy = 250 + Math.sin(node.id ?? 0) * 150
  points.value = [
    { x: cx, y: cy, name: `${node.name}-01`, channelId: `ch-${node.id}-01`, online: true },
    { x: cx + 60, y: cy + 30, name: `${node.name}-02`, channelId: `ch-${node.id}-02`, online: false }
  ]
}

function onPointClick(p: any) {
  console.log('selected channel:', p)
}

onMounted(async () => {
  try {
    const res = await getRegionTreeList()
    regions.value = (res.data as Region[]) ?? []
  } catch {
    regions.value = []
  }
})
</script>

<style scoped>
.map-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.region-card { max-height: calc(100vh - 200px); overflow: auto; }
.map-card { min-height: 540px; }
.map-stage { position: relative; padding: 12px; }
.map-svg { width: 100%; height: 500px; background: #f7fafc; border-radius: 6px; }
.map-point { cursor: pointer; transition: transform 0.15s; }
.map-point:hover { transform: scale(1.5); transform-origin: center; }
.map-legend { display: flex; gap: 16px; margin-top: 12px; font-size: 12px; color: var(--el-text-color-secondary); }
.dot { display: inline-block; width: 10px; height: 10px; border-radius: 50%; margin-right: 4px; vertical-align: middle; }
.dot-on { background: #16a34a; }
.dot-off { background: #94a3b8; }
</style>
