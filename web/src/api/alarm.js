import request from '@/utils/request'

/**
 * 查询告警列表
 * @param {Object} params - { page, count, deviceId, channelId, alarmType, alarmMethod, startTime, endTime }
 * @returns {Promise}
 */
export function listAlarms(params) {
  return request({
    url: '/api/alarm/list',
    method: 'get',
    params
  })
}

/**
 * 删除单条告警
 * @param {number} id - 告警ID
 * @returns {Promise}
 */
export function deleteAlarm(id) {
  return request({
    url: `/api/alarm/delete/${id}`,
    method: 'delete'
  })
}

/**
 * 批量删除告警
 * @param {Array<number>} ids - 告警ID 列表
 * @returns {Promise}
 */
export function batchDeleteAlarms(ids) {
  return request({
    url: '/api/alarm/batch',
    method: 'delete',
    data: { ids }
  })
}

/**
 * 处理告警
 * @param {Object} payload - { id, description }
 * @returns {Promise}
 */
export function handleAlarm(payload) {
  return request({
    url: '/api/alarm/handle',
    method: 'post',
    data: payload
  })
}

/**
 * 告警抓图
 * @param {number} id - 告警ID
 * @returns {Promise}
 */
export function snapAlarm(id) {
  return request({
    url: `/api/alarm/snap/${id}`,
    method: 'get'
  })
}

/**
 * 清空全部告警
 * @returns {Promise}
 */
export function clearAlarms() {
  return request({
    url: '/api/alarm/clear',
    method: 'delete'
  })
}