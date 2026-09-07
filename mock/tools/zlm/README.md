# ZLMediaKit Mock

模拟 ZLMediaKit 2024+ master HTTP API 的 Python 工具。

## 启动

```bash
# 默认：8080 端口，secret=demo
python3 zlm_mock.py

# 自定义 secret + 自动触发 Webhook
python3 zlm_mock.py \
  --port 8080 \
  --secret EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw \
  --hook-url http://127.0.0.1:18080/api/zlm/hook
```

注意：secret 必须与 GBServer `config/application.toml` 中 `[[zlm.servers]].secret` 字段一致。

## 实现的 API

| API | 用途 |
|-----|------|
| `/index/api/getServerConfig` | 返回 ZLM 配置（secret、协议开关） |
| `/index/api/getApiList` | 列出已实现 API |
| `/index/api/getMediaList` | 流列表（内存） |
| `/index/api/getMediaInfo` | 单个流详情 |
| `/index/api/isMediaExist` | 流是否存在 |
| `/index/api/addStreamProxy` | 添加拉流代理 |
| `/index/api/delStreamProxy` | 删除拉流代理 |
| `/index/api/openRtpServer` | 打开 RTP 接收端口 |
| `/index/api/closeRtpServer` | 关闭 RTP 接收 |
| `/index/api/listRtpServer` | RTP 列表 |
| `/index/api/getRtpInfo` | 单个 RTP 信息 |
| `/index/api/getStatistic` | 统计 |
| `/index/api/pushStream` | 触发推流 |

## 主动触发 Webhook（测试用）

```
GET /trigger/on_publish
GET /trigger/on_play
GET /trigger/on_stream_changed
GET /trigger/on_record_mp4
GET /trigger/on_record_hls
GET /trigger/on_server_started
```

每个触发器会构造对应的 Webhook 负载并 POST 到 `--hook-url` 指定的地址。

## 验证示例

```bash
# 1. 启动 mock
python3 zlm_mock.py --port 8080 --secret demo --hook-url http://127.0.0.1:18080/api/zlm/hook

# 2. 检查联通
curl -s "http://127.0.0.1:8080/index/api/getServerConfig?secret=demo" | jq .

# 3. 添加拉流代理
curl "http://127.0.0.1:8080/index/api/addStreamProxy?secret=demo&app=live&stream=test01&url=rtsp://127.0.0.1:554/stream"

# 4. 触发 on_publish Webhook（GBServer 应收到并处理）
curl "http://127.0.0.1:8080/trigger/on_publish"

# 5. 触发录像完成事件
curl "http://127.0.0.1:8080/trigger/on_record_mp4"
```

## 与 docker-compose 中真实 ZLM 的差异

| 项 | 真实 ZLM | mock |
|----|----------|------|
| 推流端口（RTSP/RTMP/HTTP-FLV） | 真实可用 | ❌ 不实现 |
| 录制落盘 | 真实文件 | ⚠️ 仅返回负载 |
| 协议转换（RTSP↔RTMP↔HLS） | 真实转码 | ❌ 不实现 |
| 多节点 / 集群 | 支持 | ❌ 单实例 |
| Webhook 事件完整 | 全部支持 | 支持 6 种（on_server_started、on_publish、on_play、on_stream_changed、on_record_mp4、on_record_hls） |

mock 主要用于**验证 GBServer → ZLM 的 HTTP 调用 + Webhook 回调路径**。

## 参数列表

| 参数 | 默认 | 说明 |
|------|------|------|
| `--host` | `0.0.0.0` | 监听地址 |
| `--port` | 8080 | 监听端口 |
| `--secret` | `demo` | ZLM API secret |
| `--hook-url` | （空） | 主动触发 Webhook 时回调 URL |
| `--log-level` | `INFO` | 日志级别 |