<template>
  <div id="alarm" class="app-container">
    <div style="height: calc(100vh - 124px);">
      <el-form :inline="true" size="mini">
        <el-form-item label="设备ID">
          <el-input
            v-model="query.deviceId"
            style="margin-right: 1rem; width: auto;"
            placeholder="设备编号"
            prefix-icon="el-icon-search"
            clearable
            @input="initData"
          />
        </el-form-item>
        <el-form-item label="通道ID">
          <el-input
            v-model="query.channelId"
            style="margin-right: 1rem; width: auto;"
            placeholder="通道编号"
            prefix-icon="el-icon-search"
            clearable
            @input="initData"
          />
        </el-form-item>
        <el-form-item label="告警类型">
          <el-input
            v-model="query.alarmType"
            style="margin-right: 1rem; width: auto;"
            placeholder="alarmType"
            prefix-icon="el-icon-search"
            clearable
            @input="initData"
          />
        </el-form-item>
        <el-form-item label="告警方式">
          <el-input
            v-model="query.alarmMethod"
            style="margin-right: 1rem; width: auto;"
            placeholder="alarmMethod"
            prefix-icon="el-icon-search"
            clearable
            @input="initData"
          />
        </el-form-item>
        <el-form-item label="开始时间">
          <el-date-picker
            v-model="startTime"
            type="datetime"
            size="mini"
            style="width: 12rem; margin-right: 1rem;"
            value-format="yyyy-MM-dd HH:mm:ss"
            placeholder="选择开始时间"
            @change="initData"
          />
        </el-form-item>
        <el-form-item label="结束时间">
          <el-date-picker
            v-model="endTime"
            type="datetime"
            size="mini"
            style="width: 12rem; margin-right: 1rem;"
            value-format="yyyy-MM-dd HH:mm:ss"
            placeholder="选择结束时间"
            @change="initData"
          />
        </el-form-item>
        <el-form-item style="float: right;">
          <el-button
            icon="el-icon-delete"
            style="margin-right: 1rem;"
            type="danger"
            :disabled="multipleSelection.length === 0"
            @click="batchDelete"
          >批量删除
          </el-button>
          <el-button
            icon="el-icon-refresh"
            @click="initData"
          >刷新
          </el-button>
        </el-form-item>
      </el-form>

      <el-table
        ref="alarmTable"
        v-loading="loading"
        :data="alarmList"
        style="width: 100%"
        height="calc(100vh - 230px)"
        border
        stripe
        @selection-change="handleSelectionChange"
      >
        <el-table-column type="selection" width="55" />
        <el-table-column prop="id" label="ID" width="80" />
        <el-table-column prop="deviceId" label="设备ID" min-width="180" show-overflow-tooltip />
        <el-table-column prop="channelId" label="通道ID" min-width="180" show-overflow-tooltip />
        <el-table-column prop="alarmPriority" label="优先级" width="80" />
        <el-table-column prop="alarmMethod" label="告警方式" min-width="120" show-overflow-tooltip />
        <el-table-column prop="alarmType" label="告警类型" min-width="120" show-overflow-tooltip />
        <el-table-column prop="alarmTime" label="告警时间" min-width="160" />
        <el-table-column prop="alarmDescription" label="告警描述" min-width="160" show-overflow-tooltip />
        <el-table-column prop="longitude" label="经度" width="100" />
        <el-table-column prop="latitude" label="纬度" width="100" />
        <el-table-column prop="createTime" label="入库时间" min-width="160" />
        <el-table-column label="操作" width="160" fixed="right">
          <template slot-scope="scope">
            <el-button size="mini" type="text" @click="handleAlarm(scope.row)">处理</el-button>
            <el-button size="mini" type="text" @click="snapAlarm(scope.row)">抓图</el-button>
            <el-button size="mini" type="text" style="color: #f56c6c" @click="deleteAlarm(scope.row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>

      <el-pagination
        :current-page.sync="query.page"
        :page-size="query.count"
        :total="total"
        :page-sizes="[10, 20, 50, 100]"
        layout="total, sizes, prev, pager, next, jumper"
        style="text-align: right; padding: 12px;"
        @size-change="initData"
        @current-change="initData"
      />
    </div>
  </div>
</template>

<script>
import { listAlarms, deleteAlarm, batchDeleteAlarms, handleAlarm } from '@/api/alarm'

export default {
  name: 'Alarm',
  data() {
    return {
      loading: false,
      alarmList: [],
      multipleSelection: [],
      total: 0,
      startTime: '',
      endTime: '',
      query: {
        page: 1,
        count: 20,
        deviceId: '',
        channelId: '',
        alarmType: '',
        alarmMethod: ''
      }
    }
  },
  created() {
    this.initData()
  },
  methods: {
    handleSelectionChange(val) {
      this.multipleSelection = val
    },
    initData: function() {
      this.loading = true
      const params = {
        page: this.query.page,
        count: this.query.count,
        deviceId: this.query.deviceId || undefined,
        channelId: this.query.channelId || undefined,
        alarmType: this.query.alarmType || undefined,
        alarmMethod: this.query.alarmMethod || undefined,
        startTime: this.startTime || undefined,
        endTime: this.endTime || undefined
      }
      listAlarms(params)
        .then((data) => {
          this.alarmList = data.list || []
          this.total = data.total || 0
        })
        .catch((error) => {
          this.$message.error('告警列表加载失败: ' + (error.msg || error.message || ''))
        })
        .finally(() => {
          this.loading = false
        })
    },
    deleteAlarm: function(row) {
      this.$confirm(`确认删除告警 ${row.id}?`, '提示', { type: 'warning' })
        .then(() => deleteAlarm(row.id))
        .then(() => {
          this.$message.success('删除成功')
          this.initData()
        })
        .catch((err) => {
          if (err !== 'cancel') {
            this.$message.error('删除失败: ' + (err.msg || err.message || ''))
          }
        })
    },
    batchDelete: function() {
      const ids = this.multipleSelection.map((s) => s.id)
      if (ids.length === 0) return
      this.$confirm(`确认批量删除 ${ids.length} 条告警?`, '提示', { type: 'warning' })
        .then(() => batchDeleteAlarms(ids))
        .then(() => {
          this.$message.success('批量删除成功')
          this.initData()
        })
        .catch((err) => {
          if (err !== 'cancel') {
            this.$message.error('批量删除失败: ' + (err.msg || err.message || ''))
          }
        })
    },
    handleAlarm: function(row) {
      this.$prompt('请输入处理说明', '处理告警 ' + row.id, { confirmButtonText: '确定', cancelButtonText: '取消' })
        .then(({ value }) => handleAlarm({ id: row.id, description: value }))
        .then(() => {
          this.$message.success('处理成功')
          this.initData()
        })
        .catch((err) => {
          if (err !== 'cancel' && err !== 'close') {
            this.$message.error('处理失败: ' + (err.msg || err.message || ''))
          }
        })
    },
    snapAlarm: function(row) {
      this.$message.info(`告警 ${row.id} 抓图请求已发起 (channel=${row.channelId})`)
    }
  }
}
</script>