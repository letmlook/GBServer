//! 设备 ↔ 平台 SIP 往返延迟（RTT）采样
//!
//! 平台周期性向每台**在线**设备发一条轻量 SIP MESSAGE（`Keepalive`，与
//! `sip/server.rs` 心跳补发用的是同一种包），用 `Call-ID` 认领设备回的
//! `200 OK`，把「发出 → 收到响应」的耗时记为一次 RTT 样本。
//!
//! 为什么不用「设备心跳到达间隔」代替：
//! 设备心跳（`Keepalive`）是**设备→平台**的单向消息，平台只能看到"多久
//! 收到一次"，看不到设备处理+回包用了多久 —— 那是心跳周期（通常 60s，
//! 由设备侧配置），不是延迟。只有平台主动发包、等设备响应，量到的才是
//! 真正的往返时间。所以延迟是**探针量出来的**，不是从心跳推出来的。
//!
//! 两个入口：
//! - [`probe_round`]：发一轮探针并登记在途（由 `sip/server.rs` 的探针循环调用）
//! - [`LatencyRegistry::on_response`]：收到响应时结算（由 `handle_response` 调用）
//!
//! 注册表是进程内全局（`OnceLock`）—— SIP 采样与 HTTP 查询在同一个进程，
//! 与 `handlers/server.rs` 的 `BUFFERS` 同一套路。延迟不落库：它是秒级变化
//! 的实时指标，持久化只会在后端重启后留下过期值；重启后一轮探针（默认 15s）
//! 就重新填满了。

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::net::UdpSocket;

use crate::config::SipConfig;

/// 探针 `Call-ID` 前缀 —— `handle_response` 靠它把响应认成延迟样本。
pub const PROBE_CALL_ID_PREFIX: &str = "lat_";

/// 参与统计的样本窗口大小（平均值/丢包率都按最近这么多次算）。
pub const WINDOW: usize = 5;

/// 一轮最多探测的设备数 —— 防止设备表异常膨胀时一轮内发出天量 MESSAGE。
const MAX_PROBES_PER_ROUND: usize = 500;

/// 探针序号，既做 `Call-ID` 去重也做 XML 里的 `SN`。
static PROBE_SEQ: AtomicU64 = AtomicU64::new(0);

/// 单台设备的延迟统计（同时是 `/api/device/query/latency` 的 JSON 结构）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceLatency {
    #[serde(rename = "deviceId")]
    pub device_id: String,
    /// 窗口内平均往返延迟（ms）。没有成功样本时为 `null`。
    #[serde(rename = "rttMs")]
    pub rtt_ms: Option<u32>,
    /// 最近一次成功的原始样本（ms）—— 失败时保留上次成功的值，便于看"掉到多少"。
    #[serde(rename = "lastMs")]
    pub last_ms: Option<u32>,
    #[serde(rename = "minMs")]
    pub min_ms: Option<u32>,
    #[serde(rename = "maxMs")]
    pub max_ms: Option<u32>,
    /// 窗口内丢包率（%）= 超时/发送失败次数 ÷ 探针总数
    #[serde(rename = "lossPct")]
    pub loss_pct: u32,
    /// 最近一次探针是否拿到成功响应
    pub ok: bool,
    /// 连续失败次数（探针发出但没等到 2xx 响应）
    #[serde(rename = "failStreak")]
    pub fail_streak: u32,
    /// 最近一次结算时间（unix 秒）
    #[serde(rename = "measuredAt")]
    pub measured_at: Option<i64>,
    /// 累计探针次数（成功 + 失败）
    pub samples: u64,
}

#[derive(Debug, Default, Clone)]
struct Entry {
    /// 最近 WINDOW 次成功的 RTT（ms）
    rtts: VecDeque<u32>,
    /// 最近 WINDOW 次探针结果，`false` = 丢包
    outcomes: VecDeque<bool>,
    last_ms: Option<u32>,
    fail_streak: u32,
    measured_at: Option<i64>,
    ok: bool,
    samples: u64,
}

#[derive(Debug)]
struct Inflight {
    device_id: String,
    started: Instant,
}

/// 进程内延迟注册表。
#[derive(Debug, Default)]
pub struct LatencyRegistry {
    /// device_id → 统计
    entries: DashMap<String, Entry>,
    /// call_id → 在途探针（发出后等响应）
    inflight: DashMap<String, Inflight>,
    /// 探针间隔（秒），0 = 未启用。只用于前端说明"多久采样一次"。
    interval_secs: AtomicU64,
}

impl LatencyRegistry {
    /// 记录探针间隔（由探针循环在启动时写入）。
    pub fn set_interval_secs(&self, secs: u64) {
        self.interval_secs.store(secs, Ordering::Relaxed);
    }

    /// 探针间隔（秒）。0 表示延迟探针未启用。
    pub fn interval_secs(&self) -> u64 {
        self.interval_secs.load(Ordering::Relaxed)
    }

    /// 登记一次在途探针。**必须在发送之前调用** —— 响应可能比登记更快回来。
    pub fn begin(&self, device_id: &str, call_id: &str) {
        self.inflight.insert(
            call_id.to_string(),
            Inflight {
                device_id: device_id.to_string(),
                started: Instant::now(),
            },
        );
    }

    /// 发送失败时撤销在途登记（这次不算样本）。
    pub fn abort(&self, call_id: &str) {
        self.inflight.remove(call_id);
    }

    /// 结算一次探针响应，返回设备 ID（没有对应在途探针时返回 `None`）。
    pub fn on_response(&self, call_id: &str, status: u16) -> Option<String> {
        let (_, probe) = self.inflight.remove(call_id)?;

        // 1xx（如 100 Trying）不是终态：重新挂回在途表，等真正的终态响应，
        // 否则往返时间会被中间响应提前结算成偏小值。
        if (100..200).contains(&status) {
            self.inflight.insert(
                call_id.to_string(),
                Inflight {
                    device_id: probe.device_id,
                    started: probe.started,
                },
            );
            return None;
        }

        let rtt_ms = probe.started.elapsed().as_millis().min(u32::MAX as u128) as u32;
        if (200..300).contains(&status) {
            self.record(&probe.device_id, Some(rtt_ms));
        } else {
            // 4xx/5xx：设备在，但拒绝了这条探针 —— 计入丢包，保留上次成功的值。
            self.record(&probe.device_id, None);
        }
        Some(probe.device_id)
    }

    /// 把超时未响应的在途探针判为丢包，返回清理条数。
    pub fn sweep_expired(&self, timeout: Duration) -> usize {
        let expired: Vec<(String, String)> = self
            .inflight
            .iter()
            .filter(|kv| kv.value().started.elapsed() > timeout)
            .map(|kv| (kv.key().clone(), kv.value().device_id.clone()))
            .collect();
        for (call_id, device_id) in &expired {
            self.inflight.remove(call_id);
            tracing::debug!("延迟探针超时：{}（{}ms 无响应）", device_id, timeout.as_millis());
            self.record(device_id, None);
        }
        expired.len()
    }

    fn record(&self, device_id: &str, rtt_ms: Option<u32>) {
        let mut e = self.entries.entry(device_id.to_string()).or_default();
        push_window(&mut e.outcomes, rtt_ms.is_some());
        match rtt_ms {
            Some(ms) => {
                push_window(&mut e.rtts, ms);
                e.last_ms = Some(ms);
                e.fail_streak = 0;
                e.ok = true;
            }
            None => {
                e.fail_streak = e.fail_streak.saturating_add(1);
                e.ok = false;
            }
        }
        e.samples = e.samples.saturating_add(1);
        e.measured_at = Some(chrono::Utc::now().timestamp());
    }

    /// 清掉某台设备的统计（设备被删除时调用，避免脏数据长留）。
    pub fn forget(&self, device_id: &str) {
        self.entries.remove(device_id);
    }

    pub fn get(&self, device_id: &str) -> Option<DeviceLatency> {
        self.entries
            .get(device_id)
            .map(|e| to_latency(device_id, &e))
    }

    /// 全部设备的延迟快照（按设备 ID 排序，输出稳定）。
    pub fn snapshot(&self) -> Vec<DeviceLatency> {
        let mut out: Vec<DeviceLatency> = self
            .entries
            .iter()
            .map(|kv| to_latency(kv.key(), &kv.value()))
            .collect();
        out.sort_by(|a, b| a.device_id.cmp(&b.device_id));
        out
    }
}

fn push_window<T>(q: &mut VecDeque<T>, v: T) {
    q.push_back(v);
    while q.len() > WINDOW {
        q.pop_front();
    }
}

fn to_latency(device_id: &str, e: &Entry) -> DeviceLatency {
    let avg = if e.rtts.is_empty() {
        None
    } else {
        let sum: u64 = e.rtts.iter().map(|v| *v as u64).sum();
        Some((sum / e.rtts.len() as u64) as u32)
    };
    let total = e.outcomes.len() as u64;
    let lost = e.outcomes.iter().filter(|ok| !**ok).count() as u64;
    DeviceLatency {
        device_id: device_id.to_string(),
        rtt_ms: avg,
        last_ms: e.last_ms,
        min_ms: e.rtts.iter().copied().min(),
        max_ms: e.rtts.iter().copied().max(),
        loss_pct: if total == 0 { 0 } else { ((lost * 100) / total) as u32 },
        ok: e.ok,
        fail_streak: e.fail_streak,
        measured_at: e.measured_at,
        samples: e.samples,
    }
}

static REGISTRY: OnceLock<LatencyRegistry> = OnceLock::new();

/// 全局延迟注册表。
pub fn registry() -> &'static LatencyRegistry {
    REGISTRY.get_or_init(LatencyRegistry::default)
}

/// 对一批在线设备发一轮探针，返回成功发出的条数。
///
/// `targets` 是 `(device_id, 信令地址)`；发送失败只记 debug 日志 ——
/// 探针是后台采样，单台失败不值得刷 warn。
pub async fn probe_round<I>(udp: &UdpSocket, config: &SipConfig, targets: I) -> usize
where
    I: IntoIterator<Item = (String, SocketAddr)>,
{
    let mut sent = 0usize;
    for (device_id, addr) in targets {
        if sent >= MAX_PROBES_PER_ROUND {
            tracing::debug!(
                "延迟探针本轮已达上限 {} 台，其余设备下一轮再探",
                MAX_PROBES_PER_ROUND
            );
            break;
        }
        match probe_device(udp, config, &device_id, addr).await {
            Ok(()) => sent += 1,
            Err(e) => tracing::debug!("延迟探针发送失败 {}@{}: {}", device_id, addr, e),
        }
    }
    sent
}

/// 向单台设备发一条延迟探针。
pub async fn probe_device(
    udp: &UdpSocket,
    config: &SipConfig,
    device_id: &str,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let seq = PROBE_SEQ.fetch_add(1, Ordering::Relaxed);
    let call_id = format!(
        "{}{}_{}_{}",
        PROBE_CALL_ID_PREFIX,
        device_id,
        chrono::Utc::now().timestamp_millis(),
        seq
    );
    let branch = format!("z9hG4bK{}", rand::random::<u32>());
    let local_tag = format!("lat{}", rand::random::<u32>());
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Message>
<CmdType>Keepalive</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
</Message>"#,
        seq % 10000,
        device_id
    );
    let uri = format!("sip:{}@{}", device_id, addr);
    let from = format!(
        "<sip:{}@{}:{}>;tag={}",
        config.device_id, config.ip, config.port, local_tag
    );
    let to = format!("<sip:{}@{}>", device_id, addr);
    let message = format!(
        "MESSAGE {uri} SIP/2.0\r\n\
         Via: SIP/2.0/UDP {ip}:{port};rport;branch={branch}\r\n\
         From: {from}\r\n\
         To: {to}\r\n\
         Call-ID: {call_id}\r\n\
         CSeq: 1 MESSAGE\r\n\
         Max-Forwards: 70\r\n\
         Content-Type: Application/MANSCDP+xml\r\n\
         Content-Length: {len}\r\n\
         \r\n\
         {body}",
        ip = config.ip,
        port = config.port,
        len = body.len(),
    );

    // 先登记再发送：局域网上设备的响应可能比登记更快回来，
    // 反过来的话这次样本会因为"没有登记过的 Call-ID"被丢掉。
    registry().begin(device_id, &call_id);

    // 经统一出站口发送：设备以 TCP 注册时，探针必须走同一条 TCP 连接
    // （走 UDP 设备收不到，样本会全变丢包）。
    if let Err(e) = crate::sip::transport::tcp::send_sip_out(udp, addr, &message).await {
        registry().abort(&call_id);
        return Err(e);
    }
    Ok(())
}
