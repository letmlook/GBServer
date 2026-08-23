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
import { getChannelList } from '@/api/channel'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'

const router = useRouter()

const regions = ref<Region[]>([])
const coord = ref<'WGS84' | 'GCJ02'>('WGS84')
const points = ref<{ x: number; y: number; name: string; channelId: string; online: boolean }[]>([])
const loadingPoints = ref(false)

async function onRegionClick(node: Region) {
  // 调 /api/common/channel/list 取该行政区划下通道，按经纬度映射到 SVG 画布
  loadingPoints.value = true
  try {
    const res = await getChannelList({ page: 1, count: 200, query: node.deviceId ?? undefined })
    const list = ((res.data as any)?.list ?? []) as Array<{
      channelId: string
      name?: string
      longitude?: number
      latitude?: number
      status?: string
    }>
    // WGS84 / GCJ02 简化为：把经度映射到 x，纬度映射到 y（中心点 + 偏移）
    // 真实地图应用高德/天地图瓦片，本 SVG 仅为示意
    const lons = list.map((c) => c.longitude ?? 0).filter((x) => x !== 0)
    const lats = list.map((c) => c.latitude ?? 0).filter((x) => x !== 0)
    const cLon = lons.length ? (Math.min(...lons) + Math.max(...lons)) / 2 : 113.27
    const cLat = lats.length ? (Math.min(...lats) + Math.max(...lats)) / 2 : 23.13
    const span = Math.max(
      ...lons.map((x) => Math.abs(x - cLon)),
      ...lats.map((y) => Math.abs(y - cLat)),
      0.1
    )
    points.value = list.map((c) => ({
      x: 400 + ((c.longitude ?? cLon) - cLon) * (300 / span),
      y: 250 - ((c.latitude ?? cLat) - cLat) * (180 / span),
      name: c.name ?? c.channelId,
      channelId: c.channelId,
      online: c.status === 'ON'
    }))
    if (points.value.length === 0) {
      ElMessage.info(`行政区划 ${node.name} 下暂未发现带经纬度的通道`)
    }
  } catch (e: any) {
    ElMessage.error(e?.message ?? '加载通道点位失败')
  } finally {
    loadingPoints.value = false
  }
}

async function onPointClick(p: any) {
  if (!p.deviceId || !p.channelId) {
    ElMessage.info('该点位缺少设备/通道标识')
    return
  }
  // 跳直播页 + 选中
  await router.push({ name: 'Live', query: { deviceId: p.deviceId, channelId: p.channelId } })
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
