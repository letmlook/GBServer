# SIP Device Mock

模拟 GB/T 28181-2016 国标设备（NVR / IPC / 平台下级）的 Python 工具。

## 启动

```bash
# 默认：连接到本地 GBServer SIP 端口
python3 sip_device_mock.py

# 自定义参数
python3 sip_device_mock.py \
  --server 127.0.0.1:5060 \
  --device-id 34020000001320000001 \
  --username admin \
  --password admin123 \
  --channels 32 \
  --auto-register \
  --auto-keepalive 30
```

## 行为验证

启动后到 `mock/logs/` 查看日志（通过 `start-mocks.sh` 启动时自动记录）。预期：

```
[INFO] sip-device-mock: SIP mock listening on 0.0.0.0:15060
[INFO] sip-device-mock: REGISTER sent (cseq=1)
[INFO] sip-device-mock: RX <- 127.0.0.1:5060: SIP/2.0 401 Unauthorized
[INFO] sip-device-mock: REGISTER (with Digest) sent (cseq=2)
[INFO] sip-device-mock: 200 OK for REGISTER
[INFO] sip-device-mock: 设备已注册到 ('127.0.0.1', 5060)
[INFO] sip-device-mock: Keepalive #1 sent
```

## 端到端场景

```bash
# 1. 启动后端
cd /Users/letmlook/code/GBServer && cargo run

# 2. 启动 SIP mock
python3 sip_device_mock.py --auto-register

# 3. 触发目录查询（通过 GBServer HTTP API）
curl -X POST http://127.0.0.1:18080/api/device/query/subscribe/catalog \
  -H "Content-Type: application/json" \
  -H "access-token: <jwt>" \
  -d '{"deviceId":"34020000001320000001"}'

# 4. 看 mock 端日志：会显示 Catalog 请求到达、响应按 8 个一分包
```

## 支持的消息

| 方向 | 消息 | 实现 |
|------|------|------|
| 上行 | REGISTER（含 Digest 重试） | ✅ |
| 上行 | 周期性 Keepalive (MESSAGE / Notify) | ✅ |
| 上行 | UNREGISTER（Expires: 0） | ✅ |
| 下行 | SUBSCRIBE | ✅ 应答 200 OK |
| 下行 | MESSAGE Catalog | ✅ 多包聚合响应 |
| 下行 | MESSAGE DeviceInfo | ✅ 响应 |
| 下行 | MESSAGE DeviceStatus | ✅ 响应 |
| 下行 | INVITE 实时 | ✅ 200 OK + SDP + 3 秒后 BYE |
| 下行 | INFO（MANSRTSP PTZ） | ✅ 200 OK |

## 参数列表

| 参数 | 默认 | 说明 |
|------|------|------|
| `--server` | `127.0.0.1:5060` | GBServer SIP 地址 |
| `--local-port` | 15060 | 本地 UDP 端口 |
| `--device-id` | `34020000001320000001` | 20 位设备 ID |
| `--device-name` | `MockCamera-01` | 设备名 |
| `--manufacturer` | `MockVendor` | 厂商 |
| `--model` | `MOCK-IPC-100` | 型号 |
| `--firmware` | `1.0.0-mock` | 固件版本 |
| `--channels` | 4 | 通道数（Catalog 多包测试用） |
| `--username` | `admin` | Digest 用户名 |
| `--password` | `admin123` | Digest 密码（**需与 config/[sip] password 一致**） |
| `--realm` | 从 device-id 推断 | SIP 域 |
| `--expires` | 3600 | 注册有效期（秒） |
| `--auto-register` | `true` | 启动后自动注册 |
| `--no-auto-register` | - | 禁用自动注册 |
| `--auto-keepalive` | 30 | 心跳间隔（秒），0 表示不发送 |

## 故障排查

- **注册死循环 401**：检查 `config/application.toml` `[sip] password` 与 `--password` 一致
- **GBServer 收不到设备**：检查 SIP mock 日志是否成功收到 200 OK
- **Catalog 收不到**：mock 端会每 8 个通道回一包，确认设备配置 `--channels` ≥ 1