# Webhook Receiver Mock

接收并记录 GBServer 主动回调的所有 Webhook 请求。

## 启动

```bash
# 默认：9090 端口
python3 webhook_receiver.py

# 自定义端口与日志
python3 webhook_receiver.py \
  --port 9090 \
  --log /Users/letmlook/code/GBServer/mock/logs/webhook.log
```

## 端点

| 端点 | 方法 | 行为 |
|------|------|------|
| `/hook/*` | POST | 通用 webhook，任意 path 都接受 |
| `/received` | GET | 返回所有已接收 webhook 的 JSON 列表 |
| `/received/clear` | GET | 清空内存中的已接收列表 |
| `/healthz` | GET | 健康检查 |

## 验证示例

```bash
# 1. 启动 mock 接收器
python3 webhook_receiver.py --port 9090

# 2. 模拟 GBServer 主动回调
curl -X POST http://127.0.0.1:9090/hook/on_publish \
  -H "Content-Type: application/json" \
  -d '{
    "hook_name": "on_publish",
    "mediaServerId": "zlmediakit-mock-1",
    "schema": "rtsp",
    "app": "rtp",
    "stream": "34020000001320000001"
  }'

# 3. 查询已接收列表
curl -s http://127.0.0.1:9090/received | jq '.[0]'

# 4. 清空
curl http://127.0.0.1:9090/received/clear
```

## 与 GBServer 的对接

将 GBServer 的 `config/application.toml` 中所有 Webhook URL 指向 mock：

```toml
[zlm]
hook_url = "http://127.0.0.1:9090/hook"

[jt1078]
retransmit_hook_url = "http://127.0.0.1:9090/hook/jt1078/retransmit"
```

然后在 GBServer 端触发 ZLM mock 或 JT1078 mock 的对应事件，到 `mock/logs/webhook.log` 验证 GBServer 是否回调成功。

## 参数列表

| 参数 | 默认 | 说明 |
|------|------|------|
| `--host` | `0.0.0.0` | 监听地址 |
| `--port` | 9090 | 监听端口 |
| `--log` | `mock/logs/webhook.log` | 持久化日志文件 |
| `--log-level` | `INFO` | 日志级别 |