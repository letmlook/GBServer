<template>
  <div id="playShare" class="app-container">
    <div v-if="loading" class="loading-mask">
      <i class="el-icon-loading"></i>
      <span style="margin-left: 8px;">正在校验分享链接…</span>
    </div>

    <el-card v-else-if="errorMsg" class="error-card">
      <div slot="header" class="error-header">
        <i class="el-icon-warning" style="color: #f56c6c; margin-right: 6px;"></i>
        <span>分享链接无效</span>
      </div>
      <p>{{ errorMsg }}</p>
      <p style="color: #909399; font-size: 13px;">链接可能已过期或被吊销，请联系分享人重新生成。</p>
    </el-card>

    <el-card v-else class="share-card">
      <div slot="header" class="share-header">
        <i class="el-icon-video-play" style="color: #409EFF; margin-right: 6px;"></i>
        <span>{{ shareInfo.deviceId }} / {{ shareInfo.channelId }}</span>
      </div>
      <div class="player-wrapper">
        <video
          v-if="playUrl"
          ref="player"
          class="video-js vjs-default-skin vjs-big-play-centered"
          controls
          preload="auto"
          :poster="snapUrl"
          style="width: 100%; height: 480px; background: #000;"
        >
          <source :src="playUrl" type="application/x-mpegURL" />
        </video>
        <div v-else class="placeholder">
          <i class="el-icon-video-camera"></i>
          <p>正在拉取播放流…</p>
        </div>
      </div>
      <div class="share-footer">
        <span style="color: #909399; font-size: 13px;">
          链接有效期至 {{ expireText }}
        </span>
        <el-button
          icon="el-icon-copy-document"
          size="mini"
          style="margin-left: 1rem;"
          @click="copyLink"
        >复制分享链接
        </el-button>
      </div>
    </el-card>
  </div>
</template>

<script>
import { shareInfo, shareStart } from '@/api/play'
import { formatExpireTime } from '@/utils/share'

export default {
  name: 'PlayShare',
  data() {
    return {
      loading: true,
      errorMsg: '',
      shareInfo: {
        deviceId: '',
        channelId: '',
        expiresAt: 0
      },
      playUrl: '',
      snapUrl: ''
    }
  },
  computed: {
    expireText() {
      return formatExpireTime(this.shareInfo.expiresAt)
    }
  },
  created() {
    this.token = this.$route.query.token || ''
    if (!this.token) {
      this.errorMsg = '缺少 token 参数'
      this.loading = false
      return
    }
    this.fetchShareInfo()
  },
  methods: {
    fetchShareInfo() {
      shareInfo({ token: this.token })
        .then((data) => {
          this.shareInfo = {
            deviceId: data.deviceId,
            channelId: data.channelId,
            expiresAt: data.expiresAt
          }
          this.startPlay()
        })
        .catch((err) => {
          this.errorMsg = err.msg || err.message || '校验失败'
        })
        .finally(() => {
          this.loading = false
        })
    },
    startPlay() {
      shareStart({ token: this.token })
        .then((data) => {
          this.playUrl = `/api/jt1078/media/stream_info_by_app_and_stream?app=${data.app}&stream=${data.stream}`
          this.snapUrl = `/api/jt1078/media/snap?app=${data.app}&stream=${data.stream}`
        })
        .catch((err) => {
          this.$message.error('启动播放失败: ' + (err.msg || err.message || ''))
        })
    },
    copyLink() {
      const url = window.location.href
      if (navigator.clipboard) {
        navigator.clipboard.writeText(url)
          .then(() => this.$message.success('链接已复制'))
          .catch(() => this.$message.error('复制失败'))
      } else {
        this.$message.warning('当前浏览器不支持一键复制，请手动复制地址栏')
      }
    }
  }
}
</script>

<style scoped>
.loading-mask {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 18px;
  color: #606266;
}
.share-card,
.error-card {
  margin: 24px;
}
.share-header,
.error-header {
  display: flex;
  align-items: center;
  font-size: 16px;
}
.player-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
}
.placeholder {
  text-align: center;
  color: #909399;
  padding: 60px 0;
}
.placeholder i {
  font-size: 64px;
}
.share-footer {
  margin-top: 16px;
  display: flex;
  align-items: center;
}
</style>