# Cascade Platform Mock

模拟 GB28181 上级平台的 Python 工具，**接收** GBServer 作为下级发送的 SIP 消息。

## 启动

```bash
# 默认：5062 端口，server-id 34020000002000000099
python3 cascade_mock.py

# 自定义参数
python3 cascade_mock.py --port 5062 --server-id 34020000002000000099
```

## 验证场景

完整级联注册场景：

```bash
# 1. 启动 mock 级联平台（5062 端口）
python3 cascade_mock.py --port 5062

# 2. 启动 GBServer

# 3. 在 GBServer HTTP API 上添加平台（注意 server_id/host/port）
curl -X POST http://127.0.0.1:18080/api/platform/add \
  -H "access-token: <jwt>" \
  -H "Content-Type: application/json" \
  -d '{
    "id": 1,
    "serverGBId": "34020000002000000099",
    "serverIp": "127.0.0.1",
    "serverPort": 5062,
    "deviceGBId": "34020000001320000001"
  }'

# 4. 观察 mock 端日志：应看到 REGISTER / Keepalive / Catalog 请求
```

## 行为

| GBServer → Mock 的请求 | mock 响应 |
|------------------------|-----------|
| REGISTER | 200 OK（**不**带 Digest 鉴权） |
| 注销（Expires: 0） | 200 OK，从已注册列表移除 |
| MESSAGE Keepalive | （不响应） |
| MESSAGE Catalog | 200 OK + 空目录响应 |
| MESSAGE DeviceInfo | 200 OK |
| INVITE | 200 OK |
| INFO（PTZ） | 200 OK |
| SUBSCRIBE / NOTIFY / BYE | 200 OK |

## 参数列表

| 参数 | 默认 | 说明 |
|------|------|------|
| `--port` | 5062 | UDP 监听端口 |
| `--server-id` | `34020000002000000099` | 上级平台 ID |
| `--realm` | `3402000000` | SIP 域 |
| `--log-level` | `INFO` | 日志级别 |

## 注意事项

- mock 平台**不实现 Digest 鉴权**；如需测试鉴权失败场景，临时修改 mock 代码即可
- 平台仅记录**入站**消息；如需主动下行 Catalog 查询，临时扩展 mock 即可
- 与 SIP device mock **端口不同**（5060 vs 5062），避免冲突