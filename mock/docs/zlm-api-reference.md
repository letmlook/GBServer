# ZLMediaKit API 子集（mock 实现）

`mock/tools/zlm/zlm_mock.py` 实现了与 ZLMediaKit 2024+ master 兼容的 API 子集。

## 已实现

| API | 类型 | 说明 |
|-----|------|------|
| `/index/api/getServerConfig` | GET | 返回 secret、协议开关 |
| `/index/api/getApiList` | GET | 返回已实现 API 列表 |
| `/index/api/getMediaList` | GET | 流列表 |
| `/index/api/getMediaInfo` | GET | 单流信息 |
| `/index/api/isMediaExist` | GET | 流是否存在 |
| `/index/api/addStreamProxy` | POST | 添加拉流代理 |
| `/index/api/delStreamProxy` | POST | 删除拉流代理 |
| `/index/api/openRtpServer` | POST | 打开 RTP 接收 |
| `/index/api/closeRtpServer` | POST | 关闭 RTP |
| `/index/api/listRtpServer` | GET | RTP 列表 |
| `/index/api/getRtpInfo` | GET | 单个 RTP 信息 |
| `/index/api/getStatistic` | GET | 统计 |
| `/index/api/pushStream` | POST | 推流 |
| `/trigger/on_publish` | GET | 主动触发 on_publish Webhook |
| `/trigger/on_play` | GET | 主动触发 on_play Webhook |
| `/trigger/on_stream_changed` | GET | 主动触发 on_stream_changed |
| `/trigger/on_record_mp4` | GET | 主动触发 on_record_mp4 |
| `/trigger/on_record_hls` | GET | 主动触发 on_record_hls |
| `/trigger/on_server_started` | GET | 主动触发 on_server_started |
| `/healthz` | GET | 健康检查 |

## 通用规范

- 所有 API 必须传 `?secret=<value>` 参数（默认 `demo`）
- 错误时返回 `{"code": <非 0>, "msg": "<描述>"}`
- 成功时返回 `{"code": 0, "msg": "success", "data": <内容>}`

## 关键 API 详解

### `openRtpServer`

请求（POST/GET 均可）：
```
POST /index/api/openRtpServer?secret=demo&port=30000&stream_id=test01&tcp_mode=0
```

成功响应（注意：`port` 与 `cookie` 在顶层，不在 `data` 里）：
```json
{
  "code": 0,
  "msg": "success",
  "port": 30000,
  "cookie": "cookie-test01"
}
```

### `addStreamProxy`

请求：
```
POST /index/api/addStreamProxy?secret=demo&app=live&stream=test01&url=rtsp://1.2.3.4/s
```

成功响应：
```json
{
  "code": 0,
  "msg": "success",
  "data": {"key": "live/test01"}
}
```

### `getMediaList`

请求：
```
GET /index/api/getMediaList?secret=demo&schema=rtsp&app=rtp&stream=34020000001320000001
```

响应：
```json
{
  "code": 0,
  "msg": "success",
  "data": [
    {
      "schema": "rtsp",
      "vhost": "__defaultVhost",
      "app": "rtp",
      "stream": "34020000001320000001",
      "duration": 0,
      "bytes_speed": 0
    }
  ]
}
```

## Webhook 事件格式

mock 主动 POST 到 `--hook-url` 时发送的负载（与 ZLM 一致）：

```json
{
  "hook_name": "on_publish",
  "mediaServerId": "zlmediakit-mock-1",
  "schema": "rtsp",
  "app": "rtp",
  "stream": "34020000001320000001",
  "ip": "127.0.0.1",
  "port": 554,
  "vhost": "__defaultVhost"
}
```

GBServer 的 `src/zlm/hook.rs` 应能正确解析（按 `hook_name` 分发）。

## 与真实 ZLM 的差异

| 项 | 真实 ZLM | mock | 备注 |
|----|----------|------|------|
| 真实媒体流（推 / 拉 / 转码） | ✅ | ❌ | mock 只在内存中跟踪流元数据 |
| 多节点 / 集群 | ✅ | ❌ | mock 单实例 |
| HLS / MP4 / WebRTC 协议转换 | ✅ | ❌ | |
| 录制落盘 | ✅ | ⚠️ 仅返回负载 | Webhook 中 file_path 是假路径 |
| `getMediaList` 性能 | O(N) | O(N) | mock 内存中遍历 |
| 鉴权（仅 secret） | ✅ | ✅ | 一致 |

mock 适合测试**协议交互路径**，不适合压测或真实播放。