# JT1078 Terminal Mock

模拟 JT/T 1078-2016 部标车辆终端的 Python 工具，基于 UDP/TCP 二进制协议。

## 启动

```bash
# 默认连接到本地 GBServer JT1078 端口
python3 jt1078_terminal_mock.py \
  --server 127.0.0.1:60000 \
  --phone-decimal 13912345678 \
  --plate 京A12345

# 模拟 5% 丢包（触发后端重传检测）
python3 jt1078_terminal_mock.py --simulate-loss 0.05
```

## 行为验证

启动后日志：

```
[INFO] jt1078-mock: JT1078 mock listening on UDP port 16000
[INFO] jt1078-mock: TX msg_id=0x0100 seq=1 body_len=76
[INFO] jt1078-mock: RX msg_id=0x0101 seq=2 body_len=68
[INFO] jt1078-mock: ✅ 注册成功
[INFO] jt1078-mock: TX msg_id=0x0002 seq=3 body_len=0     # 心跳
[INFO] jt1078-mock: TX msg_id=0x0200 seq=4 body_len=28    # 位置
```

## 支持的消息

| 消息 ID | 名称 | 方向 | 实现 |
|---------|------|------|------|
| 0x0100 | 终端注册 | 上行 | ✅ |
| 0x0101 | 注册应答 | 下行 | ✅ |
| 0x0002 | 心跳 | 上行 | ✅ |
| 0x0001 | 通用应答 | 双向 | ✅ |
| 0x0102 | 实时音视频 | 上行/下行 | ⚠️ 仅应答占位，不发实际流 |
| 0x0801 | 录像列表查询应答 | 上行 | ✅ |
| 0x8300 | 文本信息下发 | 双向 | ✅ |
| 0x1006-0x100B | 报警 | 上行 | ✅（需 `--trigger-alarm`） |
| 0x0200 | 位置上报 | 上行 | ✅（模拟北京三环移动） |
| 0x0005 | 重传请求 | 下行 | ✅（按 SEQ 重发） |
| 0x9201 | 录像列表查询 | 下行 | ✅ 应答 |

## 故障模拟能力

| 参数 | 说明 |
|------|------|
| `--simulate-loss 0.05` | 5% 上行丢包率，触发后端重传检测 |
| `--simulate-reorder 0.20` | 20% 乱序（占位） |

重传场景示例：
```bash
# 终端侧：10% 丢包
python3 jt1078_terminal_mock.py --simulate-loss 0.10

# GBServer 侧：观察 logs 中 0x0005 重传请求
RUST_LOG=gbserver=debug,gbserver::jt1078=debug cargo run
```

## 参数列表

| 参数 | 默认 | 说明 |
|------|------|------|
| `--server` | `127.0.0.1:60000` | GBServer JT1078 UDP 地址 |
| `--local-port` | 16000 | 本地 UDP 端口 |
| `--phone-decimal` | `13912345678` | 6 字节 BCD 手机号 |
| `--plate` | `京A12345` | 车牌号 |
| `--manufacturer` | `MOCK` | 厂商 ID（11 字节） |
| `--model` | `MOCK-V100` | 型号（30 字节） |
| `--device-id` | `MOCK-DEVICE-001` | 终端 ID（30 字节） |
| `--simulate-loss` | 0.0 | 丢包率（0~1） |
| `--simulate-reorder` | 0.0 | 乱序率（占位） |
| `--auto-keepalive` | 30 | 心跳间隔（秒），0 表示不发送 |
| `--auto-location` | 10 | 位置上报间隔（秒），0 表示不发送 |

## 触发报警

```python
# 在 mock 进程内通过 signal 或 IPC 触发
# （未来版本会加 HTTP 控制接口）
```

当前可以临时改造：在循环里 sleep 时随机触发报警；或者通过代码内 `mock.trigger_alarm()` 调用。