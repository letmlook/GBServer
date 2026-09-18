<template>
  <div class="map-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">电子地图</h1>
        <p class="page-subtitle">底图：高德 raster · 通道定位</p>
      </div>
      <div class="page-actions">
        <el-radio-group v-model="coord" size="small" @change="redraw">
          <el-radio-button label="GCJ02">GCJ02</el-radio-button>
          <el-radio-button label="WGS84">WGS84</el-radio-button>
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
          <div ref="mapEl" class="map-stage" />
          <div class="map-legend">
            <span><span class="dot dot-on" />在线</span>
            <span><span class="dot dot-off" />离线</span>
            <span>当前底图层级：{{ zoom }}</span>
            <span v-if="lastRegion">当前区域：{{ lastRegion.name }}（{{ points.length }} 个通道）</span>
          </div>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from 'vue'
import L from 'leaflet'
import 'leaflet/dist/leaflet.css'
import { getRegionTreeList, type Region } from '@/api/region'
import { getChannelList } from '@/api/channel'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'

const router = useRouter()

// 高德 raster 瓦片 URL 模板（GCJ02 坐标系，国内直连免 key）
// 子域名轮询提升并发；style=8 为标准矢量底图
const TILE_URL = 'https://webrd0{s}.is.autonavi.com/appmaptile?lang=zh_cn&size=1&scale=1&style=8&x={x}&y={y}&z={z}'

const regions = ref<Region[]>([])
const coord = ref<'GCJ02' | 'WGS84'>('GCJ02')
const zoom = ref<number>(5)
const points = ref<Array<{ deviceId: string; channelId: string; name: string; lng: number; lat: number; online: boolean }>>([])
const lastRegion = ref<Region | null>(null)

const mapEl = ref<HTMLDivElement | null>(null)
let map: L.Map | null = null
let markerLayer: L.LayerGroup | null = null

// ---- WGS84 → GCJ02（适用于通道坐标源为 WGS84 时） ----
const GCJ_A = 6378245.0
const GCJ_EE = 0.00669342162296594323
function outOfChina(lng: number, lat: number) {
  return lng < 72.004 || lng > 137.8347 || lat < 0.8293 || lat > 55.8271
}
function transformLat(x: number, y: number) {
  let r = -100.0 + 2.0 * x + 3.0 * y + 0.2 * y * y + 0.1 * x * y + 0.2 * Math.sqrt(Math.abs(x))
  r += ((20.0 * Math.sin(6.0 * x * Math.PI) + 20.0 * Math.sin(2.0 * x * Math.PI)) * 2.0) / 3.0
  r += ((20.0 * Math.sin(y * Math.PI) + 40.0 * Math.sin((y / 3.0) * Math.PI)) * 2.0) / 3.0
  r += ((160.0 * Math.sin((y / 12.0) * Math.PI) + 320.0 * Math.sin((y * Math.PI) / 30.0)) * 2.0) / 3.0
  return r
}
function transformLng(x: number, y: number) {
  let r = 300.0 + x + 2.0 * y + 0.1 * x * x + 0.1 * x * y + 0.1 * Math.sqrt(Math.abs(x))
  r += ((20.0 * Math.sin(6.0 * x * Math.PI) + 20.0 * Math.sin(2.0 * x * Math.PI)) * 2.0) / 3.0
  r += ((20.0 * Math.sin(x * Math.PI) + 40.0 * Math.sin((x / 3.0) * Math.PI)) * 2.0) / 3.0
  r += ((150.0 * Math.sin((x / 12.0) * Math.PI) + 300.0 * Math.sin((x / 30.0) * Math.PI)) * 2.0) / 3.0
  return r
}
function wgs84ToGcj02(lng: number, lat: number): [number, number] {
  if (outOfChina(lng, lat)) return [lng, lat]
  let dLat = transformLat(lng - 105.0, lat - 35.0)
  let dLng = transformLng(lng - 105.0, lat - 35.0)
  const radLat = (lat / 180.0) * Math.PI
  let m = Math.sin(radLat)
  m = 1 - GCJ_EE * m * m
  const sm = Math.sqrt(m)
  dLat = (dLat * 180.0) / (((GCJ_A * (1 - GCJ_EE)) / (m * sm)) * Math.PI)
  dLng = (dLng * 180.0) / ((GCJ_A / sm) * Math.cos(radLat) * Math.PI)
  return [lng + dLng, lat + dLat]
}

// 把通道坐标按用户当前坐标系规整为 GCJ02（底图坐标系）
function toMapLngLat(rawLng: number | undefined, rawLat: number | undefined): [number, number] | null {
  if (rawLng == null || rawLat == null) return null
  if (rawLng === 0 && rawLat === 0) return null
  if (coord.value === 'GCJ02') return [rawLng, rawLat]
  return wgs84ToGcj02(rawLng, rawLat)
}

function ensureMap(): L.Map {
  if (map) return map
  map = L.map(mapEl.value!, {
    center: [35.86, 104.2], // 中国几何中心附近
    zoom: 5,
    minZoom: 3,
    maxZoom: 18,
    zoomControl: true,
    attributionControl: true
  })
  L.tileLayer(TILE_URL, {
    subdomains: ['1', '2', '3', '4'],
    maxZoom: 18,
    attribution: '© <a href="https://www.amap.com" target="_blank">高德地图</a>'
  }).addTo(map)
  markerLayer = L.layerGroup().addTo(map)
  map.on('zoomend', () => {
    if (map) zoom.value = map.getZoom()
  })
  return map
}

function clearMarkers() {
  markerLayer?.clearLayers()
}

function renderMarkers() {
  ensureMap()
  clearMarkers()
  const valid: Array<[number, number]> = []
  for (const p of points.value) {
    const xy = toMapLngLat(p.lng, p.lat)
    if (!xy) continue
    const [lng, lat] = xy
    valid.push([lat, lng])
    const color = p.online ? '#16a34a' : '#94a3b8'
    const icon = L.divIcon({
      className: 'map-marker',
      html: `<span style="background:${color};border:2px solid #fff;box-shadow:0 0 0 1px rgba(0,0,0,.2);"></span>`,
      iconSize: [14, 14],
      iconAnchor: [7, 7]
    })
    const m = L.marker([lat, lng], { icon, title: p.name })
    m.bindTooltip(`${p.name} (${p.channelId})`, { direction: 'top', offset: [0, -8] })
    m.on('click', () => onPointClick(p))
    m.addTo(markerLayer!)
  }
  if (valid.length === 1) {
    map!.setView(valid[0], 14)
  } else if (valid.length > 1) {
    const bounds = L.latLngBounds(valid)
    map!.fitBounds(bounds, { padding: [40, 40], maxZoom: 16 })
  }
}

function onPointClick(p: { deviceId: string; channelId: string }) {
  if (!p.deviceId || !p.channelId) {
    ElMessage.info('该点位缺少设备/通道标识')
    return
  }
  router.push({ name: 'Live', query: { deviceId: p.deviceId, channelId: p.channelId } })
}

async function onRegionClick(node: Region) {
  lastRegion.value = node
  try {
    const res = await getChannelList({ page: 1, count: 200, query: node.deviceId ?? undefined })
    const list = ((res.data as any)?.list ?? []) as Array<{
      channelId: string
      deviceId?: string
      name?: string
      longitude?: number
      latitude?: number
      status?: string
    }>
    points.value = list.map((c) => ({
      deviceId: c.deviceId ?? '',
      channelId: c.channelId,
      name: c.name ?? c.channelId,
      lng: c.longitude ?? 0,
      lat: c.latitude ?? 0,
      online: c.status === 'ON'
    }))
    renderMarkers()
    if (points.value.length === 0) {
      ElMessage.info(`行政区划 ${node.name} 下暂未发现带经纬度的通道`)
    }
  } catch (e: any) {
    ElMessage.error(e?.message ?? '加载通道点位失败')
  }
}

// 用户切换坐标系时把现有 marker 按新坐标重画
function redraw() {
  if (points.value.length > 0) renderMarkers()
}

onMounted(async () => {
  ensureMap()
  try {
    const res = await getRegionTreeList()
    regions.value = (res.data as Region[]) ?? []
  } catch {
    regions.value = []
  }
})

onBeforeUnmount(() => {
  map?.remove()
  map = null
})
</script>

<style>
/* 全局样式：Leaflet 默认会把 marker 渲染成图片，
   这里用 divIcon + 自定义 HTML span 实现统一圆点。 */
.map-marker {
  display: flex;
  align-items: center;
  justify-content: center;
}
.map-marker > span {
  display: block;
  width: 12px;
  height: 12px;
  border-radius: 50%;
}
</style>

<style scoped>
.map-page { padding: 16px; }
.page-header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 12px; }
.page-title { font-size: 20px; font-weight: 600; margin: 0; }
.region-card { max-height: calc(100vh - 200px); overflow: auto; }
.map-card { min-height: 540px; }
.map-stage { position: relative; width: 100%; height: 540px; border-radius: 6px; overflow: hidden; }
.map-legend {
  display: flex;
  gap: 16px;
  margin-top: 12px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  flex-wrap: wrap;
}
.dot { display: inline-block; width: 10px; height: 10px; border-radius: 50%; margin-right: 4px; vertical-align: middle; }
.dot-on { background: #16a34a; }
.dot-off { background: #94a3b8; }
</style>
