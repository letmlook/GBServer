//! 语音对讲的上行音频管线：浏览器 PCM → G.711A → RTP → 设备。
//!
//! # 此前的缺口
//!
//! 对讲的 SIP 信令与 SDP 协商在 2026-09-12 之前已经修好（去话 INVITE 会登记
//! INVITE 上下文从而真正发出 ACK、设备 200 OK 的 SDP 会被解析出
//! 「平台该把音频发到哪里」），但**没有任何音频通路**：
//!
//! * 没有 G.711A（PCMA）编解码 —— 而国标对讲 SDP 写的就是
//!   `m=audio <port> RTP/AVP 8` + `a=rtpmap:8 PCMA/8000`；
//! * 没有 RTP 打包与发送实现；
//! * `TalkSession.zlm_stream_id` / `local_port` 之外没有任何消费方。
//!
//! 结果就是：信令层"通了"，但对着麦克风说话设备什么也收不到。
//!
//! # 两个方向
//!
//! * **设备 → 平台**：设备按它自己 200 OK 里宣告的端口推 RTP 到
//!   `TalkSession.local_port`（那是一个 ZLM RTP server）。浏览器直接播放
//!   ZLM 输出的 ws-flv / rtsp 即可，本模块不参与。
//! * **平台 → 设备**：由本模块负责。浏览器经 WebSocket 送 8kHz 单声道
//!   i16 PCM（小端）帧，这里编码成 G.711A、按 20ms（160 样本）打成 RTP 包，
//!   发到设备 200 OK 宣告的 `TalkSession.device_ip:device_port`。
//!
//! # 为什么是 G.711A + 20ms
//!
//! GB/T 28181 对讲默认 `PCMA/8000`；RTP 音频包常规打包周期是 20ms，
//! 即 8000 × 0.02 = **160 个样本**，对 G.711 就是 **160 字节**负载。
//! 时间戳按样本数递增（时钟频率 8000）。

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::UdpSocket;

/// 会话没有显式 SSRC 时的兜底：由 call_id 稳定派生一个 10 位十进制 SSRC
/// （前缀 4 = 音频/广播，与 `build_audio_ssrc` 的约定一致）。
///
/// 用稳定哈希而不是随机数：同一会话重连后 SSRC 不变，设备更容易关联。
pub fn fallback_ssrc(call_id: &str) -> u32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    call_id.hash(&mut hasher);
    let h = hasher.finish();
    // 4xxxxxxxxx（10 位十进制，首位 4）
    let ssrc = format!("4{:09}", h % 1_000_000_000);
    ssrc.parse::<u32>().unwrap_or(4_000_000_000)
}

/// 国标对讲使用的 RTP 负载类型：`a=rtpmap:8 PCMA/8000`。
pub const PCMA_PAYLOAD_TYPE: u8 = 8;

/// 采样率（Hz）。G.711 固定 8kHz。
pub const TALK_SAMPLE_RATE: u32 = 8000;

/// 每个 RTP 包承载的样本数（20ms @8kHz）。
pub const SAMPLES_PER_PACKET: usize = 160;

/// 每个 RTP 包承载的字节数（G.711 每样本 1 字节）。
pub const BYTES_PER_PACKET: usize = SAMPLES_PER_PACKET;

// ====================================================================
// G.711 A-law
// ====================================================================

/// A-law 各段的段尾幅值（用于查段号），取自 ITU-T G.711 / Sun g711.c。
const ALAW_SEG_END: [i32; 8] = [0x1F, 0x3F, 0x7F, 0xFF, 0x1FF, 0x3FF, 0x7FF, 0xFFF];
const ALAW_QUANT_MASK: i32 = 0x0F;
const ALAW_SEG_SHIFT: i32 = 4;

/// 单个 16bit 线性 PCM 样本 → 8bit A-law。
///
/// 与 Sun `g711.c` / ITU-T G.711 参考实现一致：先把 16bit 缩到 13bit，
/// 再按段号 + 4bit 量化，最后与符号掩码异或。A-law 的符号位为 `1` 表示正。
pub fn alaw_encode_sample(pcm: i16) -> u8 {
    let mut val = (pcm as i32) >> 3; // 16bit -> 13bit
    let mask: i32;
    if val >= 0 {
        mask = 0xD5;
    } else {
        mask = 0x55;
        // 注意用 -val-1 而不是 -val：避免 i16::MIN 取负溢出
        val = -val - 1;
    }

    // 查段号：第一个 seg_end >= val 的位置
    let mut seg = ALAW_SEG_END.len() as i32;
    for (i, &end) in ALAW_SEG_END.iter().enumerate() {
        if val <= end {
            seg = i as i32;
            break;
        }
    }

    if seg >= 8 {
        // 超出最大段，饱和到最大幅值
        return (0x7F ^ mask) as u8;
    }

    let mut aval = seg << ALAW_SEG_SHIFT;
    if seg < 2 {
        aval |= (val >> 1) & ALAW_QUANT_MASK;
    } else {
        aval |= (val >> seg) & ALAW_QUANT_MASK;
    }
    (aval ^ mask) as u8
}

/// 单个 8bit A-law → 16bit 线性 PCM。
pub fn alaw_decode_sample(a: u8) -> i16 {
    let a_val = (a as i32) ^ 0x55;
    let mut t = (a_val & ALAW_QUANT_MASK) << 4;
    let seg = (a_val & 0x70) >> ALAW_SEG_SHIFT;
    match seg {
        0 => t += 8,
        1 => t += 0x108,
        _ => {
            t += 0x108;
            t <<= seg - 1;
        }
    }
    if (a_val & 0x80) != 0 {
        t as i16
    } else {
        (-t) as i16
    }
}

/// 批量编码：16bit PCM → G.711A 字节流。
pub fn alaw_encode(pcm: &[i16]) -> Vec<u8> {
    pcm.iter().map(|&s| alaw_encode_sample(s)).collect()
}

/// 批量解码：G.711A 字节流 → 16bit PCM。
pub fn alaw_decode(bytes: &[u8]) -> Vec<i16> {
    bytes.iter().map(|&b| alaw_decode_sample(b)).collect()
}

// ====================================================================
// RTP 打包（RFC 3550）
// ====================================================================

/// RTP 打包器：按 20ms 帧输出固定 12 字节头的 RTP 包。
#[derive(Debug, Clone)]
pub struct RtpPacketizer {
    ssrc: u32,
    payload_type: u8,
    sequence: u16,
    timestamp: u32,
}

impl RtpPacketizer {
    pub fn new(ssrc: u32, payload_type: u8) -> Self {
        Self {
            ssrc,
            payload_type,
            // 起始序号/时间戳取随机量是 RFC 3550 的建议（防止明文猜测），
            // 但这里刻意从固定值开始，便于测试断言可复现。
            sequence: 0,
            timestamp: 0,
        }
    }

    pub fn ssrc(&self) -> u32 {
        self.ssrc
    }

    pub fn sequence(&self) -> u16 {
        self.sequence
    }

    pub fn timestamp(&self) -> u32 {
        self.timestamp
    }

    /// 把一段负载打成一个 RTP 包，并推进序号/时间戳。
    ///
    /// 时间戳按**样本数**递增（时钟频率 8000），因此 `payload.len()` 对
    /// G.711 恰好等于样本数。
    pub fn packetize(&mut self, payload: &[u8]) -> Vec<u8> {
        let mut pkt = Vec::with_capacity(12 + payload.len());
        // byte0: V=2, P=0, X=0, CC=0
        pkt.push(0x80);
        // byte1: M=0, PT
        pkt.push(self.payload_type & 0x7F);
        pkt.extend_from_slice(&self.sequence.to_be_bytes());
        pkt.extend_from_slice(&self.timestamp.to_be_bytes());
        pkt.extend_from_slice(&self.ssrc.to_be_bytes());
        pkt.extend_from_slice(payload);

        self.sequence = self.sequence.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(payload.len() as u32);
        pkt
    }
}

// ====================================================================
// 上行发送器
// ====================================================================

/// 对讲上行音频发送器。
///
/// 生命周期与一次对讲会话一致：会话 `Active` 后创建，BYE/停止时 drop 即可
/// （socket 随之关闭）。
#[derive(Debug)]
pub struct TalkAudioSender {
    socket: UdpSocket,
    target: SocketAddr,
    /// 序列号 / 时间戳状态。锁只在**同步地打包**期间持有，
    /// 不在 `send().await` 期间持有。
    packetizer: std::sync::Mutex<RtpPacketizer>,
    packets_sent: AtomicU64,
    payload_bytes_sent: AtomicU64,
}

impl TalkAudioSender {
    /// 绑定一个本地 UDP socket 并指向设备音频地址。
    ///
    /// `device_ip` 来自设备 200 OK 的 SDP（若设备写 `0.0.0.0`，
    /// 调用方应已回退成设备的信令源地址，见 `sip::server::handle_response`）。
    pub async fn connect(device_ip: &str, device_port: u16, ssrc: u32) -> std::io::Result<Self> {
        let target: SocketAddr = format!("{}:{}", device_ip, device_port)
            .parse()
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("非法设备音频地址 {}:{}: {}", device_ip, device_port, e),
                )
            })?;
        // 绑定 0.0.0.0:0 由内核选源端口
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(target).await?;
        Ok(Self {
            socket,
            target,
            packetizer: std::sync::Mutex::new(RtpPacketizer::new(ssrc, PCMA_PAYLOAD_TYPE)),
            packets_sent: AtomicU64::new(0),
            payload_bytes_sent: AtomicU64::new(0),
        })
    }

    pub fn target(&self) -> SocketAddr {
        self.target
    }

    pub fn ssrc(&self) -> u32 {
        self.packetizer
            .lock()
            .map(|g| g.ssrc())
            .unwrap_or_default()
    }

    /// 已发送 (RTP 包数, G.711 负载字节数)。
    pub fn stats(&self) -> (u64, u64) {
        (
            self.packets_sent.load(Ordering::Relaxed),
            self.payload_bytes_sent.load(Ordering::Relaxed),
        )
    }

    /// 发送一段 8kHz 单声道 PCM（内部编码为 G.711A 后按 160 字节切帧）。
    ///
    /// 不足一帧的尾包**也发**，否则最后一个短帧会被静默丢掉，
    /// 听感上就是句尾被吞。返回实际发送的 RTP 包数。
    pub async fn send_pcm_8k(&self, pcm: &[i16]) -> std::io::Result<usize> {
        let alaw = alaw_encode(pcm);
        self.send_alaw_frames(&alaw).await
    }

    /// 发送已经编码好的 G.711A 字节流（按 160 字节切帧）。
    ///
    /// 先把整批包**同步打包**好（此时才持有锁，保证序号/时间戳在批内连续递增），
    /// 再逐个 `await` 发出 —— 锁不会跨 await 持有。
    pub async fn send_alaw_frames(&self, alaw: &[u8]) -> std::io::Result<usize> {
        if alaw.is_empty() {
            return Ok(0);
        }
        let packets: Vec<Vec<u8>> = {
            let mut pk = self
                .packetizer
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            alaw.chunks(BYTES_PER_PACKET)
                .map(|frame| pk.packetize(frame))
                .collect()
        };

        for pkt in &packets {
            self.socket.send(pkt).await?;
        }
        self.packets_sent
            .fetch_add(packets.len() as u64, Ordering::Relaxed);
        self.payload_bytes_sent
            .fetch_add(alaw.len() as u64, Ordering::Relaxed);
        Ok(packets.len())
    }
}

// ====================================================================
// 单元测试
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alaw_known_vectors() {
        // 期望值来自 Sun g711.c / ITU-T G.711 参考实现（linear2alaw / alaw2linear），
        // 由**独立转写**的 Python 参考实现算出后固化，用于交叉核对 Rust 侧转写。
        let encode_cases: &[(i16, u8)] = &[
            (0, 0xD5),
            (1, 0xD5),
            (8, 0xD5),
            (16, 0xD4),
            (-1, 0x55),
            (-8, 0x55),
            (1000, 0xFA),
            (-1000, 0x7A),
            (30000, 0xA8),
            (-30000, 0x28),
            (i16::MAX, 0xAA),
            (i16::MIN, 0x2A),
        ];
        for &(pcm, want) in encode_cases {
            assert_eq!(
                alaw_encode_sample(pcm),
                want,
                "linear2alaw({}) 应为 0x{:02X}",
                pcm,
                want
            );
        }

        // A-law 的符号位：bit7 = 1 表示正
        assert_eq!(alaw_encode_sample(1000) & 0x80, 0x80);
        assert_eq!(alaw_encode_sample(-1000) & 0x80, 0x00);

        // 解码侧同样以参考实现为准。
        // 注意：A-law 的数字静音 0 编码为 0xD5，而 0xD5 解码回去是 **8**（不是 0）——
        // 这是 A-law 固有的小直流偏置（零交叉落在 0xD5 与 0x55 之间），不是 bug。
        let decode_cases: &[(u8, i16)] = &[
            (0xD5, 8),
            (0x55, -8),
            (0xD4, 24),
            (0xFA, 1008),
            (0x7A, -1008),
            (0xA8, 30208),
            (0x28, -30208),
            (0xAA, 32256),
            (0x2A, -32256),
        ];
        for &(code, want) in decode_cases {
            assert_eq!(
                alaw_decode_sample(code),
                want,
                "alaw2linear(0x{:02X}) 应为 {}",
                code,
                want
            );
        }
    }

    #[test]
    fn alaw_roundtrip_error_is_bounded() {
        // 覆盖全量 i16 范围的一个密集采样；A-law 是有损压缩，
        // 只要求误差落在该段的量化步长内（≈ 幅值的 1/8 + 常数）。
        for x in (-32768i32..=32767).step_by(7) {
            let x = x as i16;
            let back = alaw_decode_sample(alaw_encode_sample(x)) as i32;
            let err = (back - x as i32).abs();
            let bound = (x as i32).abs() / 8 + 20;
            assert!(
                err <= bound,
                "x={} decode(encode(x))={} err={} bound={}",
                x,
                back,
                err,
                bound
            );
        }
    }

    #[test]
    fn alaw_code_is_monotonic_after_unmasking() {
        // 规范性质：去掉 0x55 掩码后的 7 位码值随幅值单调递增
        // （原始字节因掩码异或并不单调，所以必须在 ^0x55 之后比较）。
        let mut prev: i32 = -1;
        for x in (0i32..=32000).step_by(7) {
            let code = (alaw_encode_sample(x as i16) ^ 0x55) as i32;
            assert!(
                code >= prev,
                "x={} code^0x55={} 小于前值 {}",
                x,
                code,
                prev
            );
            prev = code;
        }
        // 解码幅值随**未加掩码**的 7 位码值单调（正半轴 0x80..=0xFF）。
        // 不能按原始字节比较：量化位在原始字节里不是幅值顺序
        // （例：0xD6 -> 56 而 0xD7 -> 40）。
        let mut prev_mag = -1i32;
        for unmasked in 0x80u8..=0xFF {
            let code = unmasked ^ 0x55;
            let mag = alaw_decode_sample(code) as i32;
            assert!(
                mag >= prev_mag,
                "未掩码码 {:#04X} (byte {:#04X}) -> {} 小于前值 {}",
                unmasked,
                code,
                mag,
                prev_mag
            );
            prev_mag = mag;
        }
    }

    #[test]
    fn alaw_encode_batch_and_silence() {
        let silence = vec![0i16; SAMPLES_PER_PACKET];
        let bytes = alaw_encode(&silence);
        assert_eq!(bytes.len(), BYTES_PER_PACKET);
        assert!(bytes.iter().all(|&b| b == 0xD5), "静音应全部编码为 0xD5");

        let decoded = alaw_decode(&bytes);
        assert_eq!(decoded.len(), SAMPLES_PER_PACKET);
        // A-law 数字静音编码为 0xD5，解码回来是 +8（固有直流偏置，见上）
        assert!(decoded.iter().all(|&s| s == 8));
    }

    #[test]
    fn rtp_header_layout_and_counters() {
        let mut pk = RtpPacketizer::new(0xDEADBEEF, PCMA_PAYLOAD_TYPE);
        let payload = vec![0xD5u8; BYTES_PER_PACKET];
        let pkt = pk.packetize(&payload);

        assert_eq!(pkt.len(), 12 + BYTES_PER_PACKET);
        assert_eq!(pkt[0], 0x80, "V=2,P=0,X=0,CC=0");
        assert_eq!(pkt[1] & 0x7F, PCMA_PAYLOAD_TYPE, "PT 必须为 8 (PCMA)");
        assert_eq!(u16::from_be_bytes([pkt[2], pkt[3]]), 0, "首个序号为 0");
        assert_eq!(
            u32::from_be_bytes([pkt[4], pkt[5], pkt[6], pkt[7]]),
            0,
            "首个时间戳为 0"
        );
        assert_eq!(
            u32::from_be_bytes([pkt[8], pkt[9], pkt[10], pkt[11]]),
            0xDEADBEEF,
            "SSRC 必须按大端写入"
        );
        assert_eq!(&pkt[12..], &payload[..]);

        // 序号 +1，时间戳 +样本数
        let pkt2 = pk.packetize(&payload);
        assert_eq!(u16::from_be_bytes([pkt2[2], pkt2[3]]), 1);
        assert_eq!(
            u32::from_be_bytes([pkt2[4], pkt2[5], pkt2[6], pkt2[7]]),
            SAMPLES_PER_PACKET as u32
        );
    }

    /// 端到端：真的用一个 UDP socket 当"设备"，验证收到的确实是
    /// 合法 RTP + G.711A 静音，且分帧、序号、时间戳、SSRC 都对。
    #[tokio::test]
    async fn sender_delivers_valid_rtp_to_device_socket() {
        let device = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let device_addr = device.local_addr().unwrap();

        let sender = TalkAudioSender::connect("127.0.0.1", device_addr.port(), 0x01020304)
            .await
            .unwrap();
        assert_eq!(sender.ssrc(), 0x01020304);

        // 320 个样本 = 2 个 20ms 帧
        let pcm = vec![0i16; 2 * SAMPLES_PER_PACKET];
        let sent = sender.send_pcm_8k(&pcm).await.unwrap();
        assert_eq!(sent, 2, "320 样本应切成 2 个 RTP 包");

        let mut buf = [0u8; 2048];
        for expect_seq in 0u16..2 {
            let n = tokio::time::timeout(std::time::Duration::from_secs(2), device.recv(&mut buf))
                .await
                .expect("等待设备侧收包超时")
                .unwrap();
            assert_eq!(n, 12 + BYTES_PER_PACKET, "RTP 头 12 字节 + 160 字节负载");
            assert_eq!(buf[0], 0x80, "版本位必须是 2");
            assert_eq!(buf[1] & 0x7F, PCMA_PAYLOAD_TYPE);
            assert_eq!(u16::from_be_bytes([buf[2], buf[3]]), expect_seq);
            assert_eq!(
                u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]),
                (expect_seq as u32) * SAMPLES_PER_PACKET as u32,
                "时间戳按样本数递增"
            );
            assert_eq!(
                u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
                0x01020304
            );
            assert!(
                buf[12..n].iter().all(|&b| b == 0xD5),
                "静音负载必须是 0xD5"
            );
        }

        let (packets, bytes) = sender.stats();
        assert_eq!(packets, 2);
        assert_eq!(bytes, (2 * BYTES_PER_PACKET) as u64);
    }

    /// 非 20ms 对齐的尾包也必须发出去，不能被静默丢弃。
    #[tokio::test]
    async fn sender_sends_partial_tail_frame() {
        let device = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let device_addr = device.local_addr().unwrap();
        let sender = TalkAudioSender::connect("127.0.0.1", device_addr.port(), 7)
            .await
            .unwrap();

        // 170 样本 = 1 整帧 + 10 样本尾包
        let pcm = vec![100i16; 170];
        assert_eq!(sender.send_pcm_8k(&pcm).await.unwrap(), 2);

        let mut buf = [0u8; 2048];
        let n1 = device.recv(&mut buf).await.unwrap();
        assert_eq!(n1, 12 + BYTES_PER_PACKET);
        let n2 = device.recv(&mut buf).await.unwrap();
        assert_eq!(n2, 12 + 10, "尾包应只带 10 字节负载");
    }

    #[tokio::test]
    async fn connect_rejects_invalid_ip() {
        let err = TalkAudioSender::connect("not-an-ip", 10000, 1).await;
        assert!(err.is_err());
    }
}
