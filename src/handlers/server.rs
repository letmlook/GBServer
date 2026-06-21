//! 流媒体服务器与系统配置 API，与前端 server.js 对应

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use crate::db::{get_media_server_by_id, list_media_servers, media_server, stream_push, stream_proxy, MediaServer};
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::state::StreamState;

use crate::AppState;

use chrono::TimeZone;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::json;
use std::str::FromStr;
use crate::db as db;

// Helper functions to read system metrics
#[cfg(any(target_os = "linux", target_os = "macos"))]
use {std::fs::File, std::io::Read, tokio::time::sleep, std::time::Duration};
use std::process::Command;

pub async fn zlm_proxy(
    method: Method,
    State(state): State<AppState>,
    Path((media_server_id, path)): Path<(String, String)>,
    Query(mut params): Query<HashMap<String, String>>,
    body: Bytes,
) -> Response {
    let Some(client) = state.get_zlm_client(Some(&media_server_id)) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "code": 404,
                "msg": format!("媒体服务不存在: {}", media_server_id),
                "data": null
            })),
        )
            .into_response();
    };

    params
        .entry("secret".to_string())
        .or_insert_with(|| client.secret.clone());

    let target = format!("{}/{}", client.base_url().trim_end_matches('/'), path);
    let http = reqwest::Client::new();
    let req = if method == Method::POST {
        http.post(&target).query(&params).body(body)
    } else {
        http.get(&target).query(&params)
    };
    let resp = match req.send().await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!("ZLM proxy request failed: {} -> {}", target, e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "code": 502,
                    "msg": format!("ZLM请求失败: {}", e),
                    "data": null
                })),
            )
                .into_response();
        }
    };

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| header::HeaderValue::from_str(value).ok())
        .unwrap_or_else(|| header::HeaderValue::from_static("application/json"));
    let body = match resp.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "code": 502,
                    "msg": format!("读取ZLM响应失败: {}", e),
                    "data": null
                })),
            )
                .into_response();
        }
    };

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .body(axum::body::Body::from(body))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}

async fn configure_zlm_hooks(
    state: &AppState,
    media_server_id: &str,
    client: &crate::zlm::ZlmClient,
) -> Vec<String> {
    let hook_url = state
        .config
        .zlm
        .as_ref()
        .and_then(|cfg| {
            cfg.servers
                .iter()
                .find(|server| server.id == media_server_id)
                .and_then(|server| server.hook_url.clone())
                .or_else(|| Some(cfg.hook_url.clone()))
        })
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| format!("http://127.0.0.1:{}/api/zlm/hook", state.config.server.port));

    let config_items = [
        ("hook.enable", "1".to_string()),
        ("hook.on_server_started", hook_url.clone()),
        ("hook.on_server_keepalive", hook_url.clone()),
        ("hook.on_stream_changed", hook_url.clone()),
        ("hook.on_stream_not_found", hook_url.clone()),
        ("hook.on_record_mp4", hook_url.clone()),
        ("hook.on_record_hls", hook_url.clone()),
        ("hook.on_publish", hook_url.clone()),
        ("hook.on_play", hook_url.clone()),
        ("hook.on_flow_report", hook_url.clone()),
        ("hook.on_rtp_server_timeout", hook_url.clone()),
    ];

    let mut errors = Vec::new();
    for (key, value) in config_items {
        if let Err(e) = client.set_server_config(&client.secret, key, &value).await {
            let msg = format!("{}={}: {}", key, value, e);
            tracing::warn!("Failed to configure ZLM hook {}", msg);
            errors.push(msg);
        }
    }
    errors
}

// ── Platform-agnostic CPU usage ──
async fn read_cpu_usage() -> Option<f64> {
    read_cpu_usage_impl().await
}

#[cfg(target_os = "linux")]
async fn read_cpu_usage_impl() -> Option<f64> {
    let (t1, id1) = read_cpu_times()?;
    sleep(Duration::from_millis(60)).await;
    let (t2, id2) = read_cpu_times()?;
    let dt = t2.saturating_sub(t1) as f64;
    let di = id2.saturating_sub(id1) as f64;
    if dt <= 0.0 { return Some(0.0); }
    Some(((dt - di) / dt) * 100.0)
}
#[cfg(target_os = "linux")]
fn read_cpu_times() -> Option<(u64, u64)> {
    let mut s = String::new();
    File::open("/proc/stat").ok()?.read_to_string(&mut s).ok()?;
    for line in s.lines() {
        if line.starts_with("cpu ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let vals: Vec<u64> = parts.iter().skip(1).filter_map(|p| p.parse().ok()).collect();
            let user = *vals.first().unwrap_or(&0);
            let nice = *vals.get(1).unwrap_or(&0);
            let system = *vals.get(2).unwrap_or(&0);
            let idle = *vals.get(3).unwrap_or(&0);
            let iowait = *vals.get(4).unwrap_or(&0);
            let total: u64 = [user, nice, system, idle, iowait].iter()
                .chain(vals.get(5..).unwrap_or(&[])).sum();
            return Some((total, idle + iowait));
        }
    }
    None
}

#[cfg(target_os = "macos")]
async fn read_cpu_usage_impl() -> Option<f64> {
    // Use `top -l 1 -n 0` to get CPU usage
    let output = Command::new("top")
        .args(["-l", "1", "-n", "0"])
        .output().ok()?;
    let out = String::from_utf8_lossy(&output.stdout);
    for line in out.lines() {
        if line.contains("CPU usage:") {
            // Format: "CPU usage: 5.26% user, 10.52% sys, 84.21% idle"
            let parts: Vec<&str> = line.split_whitespace().collect();
            for (i, p) in parts.iter().enumerate() {
                if p.contains("idle") && i > 0 {
                    if let Ok(idle_pct) = parts[i-1].trim_end_matches('%').parse::<f64>() {
                        return Some((100.0 - idle_pct).max(0.0));
                    }
                }
            }
        }
    }
    // Fallback: use `ps` to approximate
    let output = Command::new("ps")
        .args(["-A", "-o", "%cpu"])
        .output().ok()?;
    let out = String::from_utf8_lossy(&output.stdout);
    let total: f64 = out.lines().skip(1)
        .filter_map(|l| l.trim().parse::<f64>().ok())
        .sum();
    Some(total.min(100.0))
}

#[cfg(target_os = "windows")]
async fn read_cpu_usage_impl() -> Option<f64> {
    // Use wmic to get CPU load
    let output = Command::new("wmic")
        .args(["cpu", "get", "loadpercentage"])
        .output().ok()?;
    let out = String::from_utf8_lossy(&output.stdout);
    out.lines().skip(1)
        .find_map(|l| l.trim().parse::<f64>().ok())
        .or(Some(5.0))
}

// ── Platform-agnostic memory info ──
fn read_memory_info() -> Option<(u64, u64, u64)> { read_memory_info_impl() }

#[cfg(target_os = "linux")]
fn read_memory_info_impl() -> Option<(u64, u64, u64)> {
    let mut s = String::new();
    File::open("/proc/meminfo").ok()?.read_to_string(&mut s).ok()?;
    let mut total = 0u64;
    let mut avail = 0u64;
    for line in s.lines() {
        if line.starts_with("MemTotal:") {
            total = line.split_whitespace().nth(1)?.parse::<u64>().ok()? * 1024;
        } else if line.starts_with("MemAvailable:") {
            avail = line.split_whitespace().nth(1)?.parse::<u64>().ok()? * 1024;
        }
    }
    Some((total, avail, total.saturating_sub(avail)))
}

#[cfg(target_os = "macos")]
fn read_memory_info_impl() -> Option<(u64, u64, u64)> {
    // Total from sysctl
    let total_out = Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()?;
    let total: u64 = String::from_utf8_lossy(&total_out.stdout).trim().parse().ok()?;
    // Usage from vm_stat (page size is 4096 on Apple Silicon, 16384 on Intel)
    let page_size_out = Command::new("sysctl").args(["-n", "hw.pagesize"]).output().ok()?;
    let page_size: u64 = String::from_utf8_lossy(&page_size_out.stdout).trim().parse().unwrap_or(16384);
    let vm_out = Command::new("vm_stat").output().ok()?;
    let vm = String::from_utf8_lossy(&vm_out.stdout);
    let mut active_pages = 0u64;
    let mut inactive_pages = 0u64;
    let mut wired_pages = 0u64;
    for line in vm.lines() {
        if line.contains("Pages active:") {
            active_pages = line.split(':').nth(1)?.trim().trim_end_matches('.').parse().ok()?;
        } else if line.contains("Pages inactive:") {
            inactive_pages = line.split(':').nth(1)?.trim().trim_end_matches('.').parse().ok()?;
        } else if line.contains("Pages wired down:") {
            wired_pages = line.split(':').nth(1)?.trim().trim_end_matches('.').parse().ok()?;
        }
    }
    let used_pages = active_pages + wired_pages + (inactive_pages / 2);
    let used = used_pages * page_size;
    Some((total, total.saturating_sub(used), used))
}

#[cfg(target_os = "windows")]
fn read_memory_info_impl() -> Option<(u64, u64, u64)> {
    // Fallback: return dummy values; real impl would use GlobalMemoryStatusEx via winapi
    Some((16 * 1024 * 1024 * 1024, 8 * 1024 * 1024 * 1024, 8 * 1024 * 1024 * 1024))
}

// ── Platform-agnostic disk usage ──
fn read_disk_usage() -> Option<(u64, u64, u64)> { read_disk_usage_impl() }

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn read_disk_usage_impl() -> Option<(u64, u64, u64)> {
    let output = Command::new("df").arg("-k").arg("/").output().ok()?;
    if !output.status.success() { return None; }
    let out = String::from_utf8_lossy(&output.stdout);
    for (idx, line) in out.lines().enumerate() {
        if idx == 1 {
            let mut it = line.split_whitespace();
            let _fs = it.next();
            let total_kb = it.next()?.parse::<u64>().ok()?;
            let used_kb = it.next()?.parse::<u64>().ok()?;
            return Some((total_kb * 1024, used_kb * 1024, 0));
        }
    }
    None
}

/// 列出真实磁盘（每根 = 一个物理磁盘），返回 `(path, total_bytes, used_bytes)`。
///
/// dashboard 磁盘图期望多根柱子（每根 = 一个真实物理磁盘）。我们解析
/// `df -kP` 输出，按以下策略过滤与去重：
/// - macOS: 只保留 `/dev/diskN[sM]` 这类真实块设备；tmpfs/devfs/map
///   /overlay/squashfs/aufs 等伪文件系统直接跳过。
/// - Linux: 只保留 `/dev/sd*`、`/dev/nvme*`、`/dev/vd*`、`/dev/xvd*`、
///   `/dev/hd*` 这类块设备。
///
/// 去重原则：
/// 1. 同一"物理磁盘族"的多个挂载点合并为 1 根柱
///    (e.g. `/dev/disk3s1`/`s3`/`s6` → disk3；
///          `/dev/sda1`/`sda2` → sda；
///          `/dev/nvme0n1p1`/`p2` → nvme0n1)
/// 2. 模拟器 dmg 镜像（典型 < 30 GB）和 tmp 数据盘会被容量阈值过滤，
///    只在 dashboard 显示"真实"磁盘。
/// 3. 主挂载点 `/` 优先作为该磁盘的代表挂载点；其余情况下选使用率
///    最高的挂载点（更直观反映磁盘压力）。
fn read_all_disk_usage() -> Vec<(String, u64, u64)> {
    let output = match Command::new("df").arg("-kP").output() {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let out = String::from_utf8_lossy(&output.stdout);

    const MIN_DISK_BYTES: u64 = 50 * 1024 * 1024 * 1024; // 50 GB

    // 收集通过基础过滤的行
    let mut rows: Vec<(String, String, u64, u64)> = Vec::new();
    for (idx, line) in out.lines().enumerate() {
        if idx == 0 { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 6 { continue; }
        let fs = parts[0];
        #[cfg(target_os = "macos")]
        {
            if !fs.starts_with("/dev/disk") { continue; }
        }
        #[cfg(target_os = "linux")]
        {
            if !(fs.starts_with("/dev/sd")
                || fs.starts_with("/dev/nvme")
                || fs.starts_with("/dev/vd")
                || fs.starts_with("/dev/xvd")
                || fs.starts_with("/dev/hd"))
            {
                continue;
            }
        }
        let total_kb = match parts[1].parse::<u64>() { Ok(n) => n, Err(_) => continue };
        let used_kb  = match parts[2].parse::<u64>() { Ok(n) => n, Err(_) => continue };
        let mount = parts.last().unwrap_or(&"/").to_string();
        rows.push((fs.to_string(), mount, total_kb, used_kb));
    }

    // 取每个 disk family 的代表挂载点；按容量阈值过滤
    use std::collections::HashMap;
    let mut by_disk: HashMap<String, (String, u64, u64)> = HashMap::new();
    for (fs, mount, total_kb, used_kb) in rows {
        let family = disk_family(fs.as_str());
        let total_bytes = total_kb * 1024;
        // 容量阈值过滤：忽略 dmg / 模拟器 / 小数据盘
        if total_bytes < MIN_DISK_BYTES { continue; }

        let entry = by_disk.entry(family).or_insert_with(|| {
            (mount.clone(), total_kb, used_kb)
        });
        if mount == "/" {
            entry.0 = mount;
            entry.1 = total_kb;
            entry.2 = used_kb;
        } else if entry.0 != "/" && used_kb > entry.2 {
            entry.0 = mount;
            entry.1 = total_kb;
            entry.2 = used_kb;
        }
    }

    let mut result: Vec<(String, u64, u64)> = by_disk.into_iter()
        .map(|(_, (m, t, u))| (m, t * 1024, u * 1024))
        .collect();
    result.sort_by(|a, b| {
        if a.0 == "/" { return std::cmp::Ordering::Less; }
        if b.0 == "/" { return std::cmp::Ordering::Greater; }
        a.0.cmp(&b.0)
    });
    result
}

/// 把块设备名归一化为"物理磁盘族"标识：
/// - `/dev/disk3s5` → `disk3` （去掉 `s[0-9]+` 后缀；APFS volume）
/// - `/dev/disk0s2` → `disk0`
/// - `/dev/sda2` → `sda` （去掉末尾数字）
/// - `/dev/nvme0n1p2` → `nvme0n1`
/// - `/dev/disk3s3s1` → `disk3` （去掉所有 `s[0-9]+` 后缀；APFS snapshot）
fn disk_family(fs: &str) -> String {
    let s = fs.trim_start_matches("/dev/");
    // macOS APFS 子卷 / snapshot 后缀：去掉所有 sN 后缀
    let s = if let Some(stripped) = strip_apfs_suffixes(s) {
        stripped
    } else {
        s.to_string()
    };
    // Linux: 去掉末尾纯数字部分（sda1 → sda）
    let family: String = s.chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    family
}

fn strip_apfs_suffixes(s: &str) -> Option<String> {
    // 反复去掉末尾的 `s[0-9]+`
    let mut cur = s.to_string();
    let mut changed = false;
    loop {
        if let Some(pos) = cur.rfind('s') {
            let after = &cur[pos + 1..];
            if !after.is_empty() && after.chars().all(|c| c.is_ascii_digit()) {
                cur.truncate(pos);
                changed = true;
                continue;
            }
        }
        break;
    }
    if changed { Some(cur) } else { None }
}

#[cfg(target_os = "windows")]
fn read_all_disk_usage() -> Vec<(String, u64, u64)> {
    // Fallback: single synthetic disk
    vec![("C:".to_string(),
          100 * 1024 * 1024 * 1024,
          50 * 1024 * 1024 * 1024)]
}

#[cfg(target_os = "windows")]
fn read_disk_usage_impl() -> Option<(u64, u64, u64)> {
    Some((100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024))
}

// ── Platform-agnostic uptime ──
fn read_uptime() -> Option<f64> { read_uptime_impl() }

#[cfg(target_os = "linux")]
fn read_uptime_impl() -> Option<f64> {
    let mut s = String::new();
    File::open("/proc/uptime").ok()?.read_to_string(&mut s).ok()?;
    s.split_whitespace().next()?.parse::<f64>().ok()
}

#[cfg(target_os = "macos")]
fn read_uptime_impl() -> Option<f64> {
    // Get boot time via sysctl, compute uptime
    let output = Command::new("sysctl").args(["-n", "kern.boottime"]).output().ok()?;
    let out = String::from_utf8_lossy(&output.stdout);
    // Format: { sec = 1234567890, usec = 123456 } Fri May  1 12:00:00 2026
    let boot_secs = out.split("sec = ")
        .nth(1)?
        .split(',')
        .next()?
        .trim()
        .parse::<u64>().ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    Some(now.as_secs().saturating_sub(boot_secs) as f64)
}

#[cfg(target_os = "windows")]
fn read_uptime_impl() -> Option<f64> {
    Some(3600.0)
}

// ── Platform-agnostic network I/O ──
async fn read_network_rx_mbps() -> Option<f64> { read_net_rx_impl().await }
async fn read_network_tx_mbps() -> Option<f64> { read_net_tx_impl().await }

#[cfg(target_os = "linux")]
async fn read_net_rx_impl() -> Option<f64> {
    let (rx1, _) = read_net_dev_bytes()?;
    sleep(Duration::from_millis(100)).await;
    let (rx2, _) = read_net_dev_bytes()?;
    Some(((rx2.saturating_sub(rx1)) as f64 * 8.0 / 0.1) / 1_000_000.0)
}

#[cfg(target_os = "linux")]
async fn read_net_tx_impl() -> Option<f64> {
    let (_, tx1) = read_net_dev_bytes()?;
    sleep(Duration::from_millis(100)).await;
    let (_, tx2) = read_net_dev_bytes()?;
    Some(((tx2.saturating_sub(tx1)) as f64 * 8.0 / 0.1) / 1_000_000.0)
}

#[cfg(target_os = "linux")]
fn read_net_dev_bytes() -> Option<(u64, u64)> {
    let mut s = String::new();
    File::open("/proc/net/dev").ok()?.read_to_string(&mut s).ok()?;
    let mut rx_total = 0u64;
    let mut tx_total = 0u64;
    for line in s.lines().skip(2) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 10 && parts[0].contains(':') {
            rx_total += parts[1].parse::<u64>().unwrap_or(0);
            tx_total += parts[9].parse::<u64>().unwrap_or(0);
        }
    }
    Some((rx_total, tx_total))
}

#[cfg(target_os = "macos")]
async fn read_net_rx_impl() -> Option<f64> {
    let (rx1, _) = read_netstat_bytes()?;
    sleep(Duration::from_millis(100)).await;
    let (rx2, _) = read_netstat_bytes()?;
    Some(((rx2.saturating_sub(rx1)) as f64 * 8.0 / 0.1) / 1_000_000.0)
}

#[cfg(target_os = "macos")]
async fn read_net_tx_impl() -> Option<f64> {
    let (_, tx1) = read_netstat_bytes()?;
    sleep(Duration::from_millis(100)).await;
    let (_, tx2) = read_netstat_bytes()?;
    Some(((tx2.saturating_sub(tx1)) as f64 * 8.0 / 0.1) / 1_000_000.0)
}

#[cfg(target_os = "macos")]
fn read_netstat_bytes() -> Option<(u64, u64)> {
    // netstat -ib shows network interface stats
    let output = Command::new("netstat").args(["-ib"]).output().ok()?;
    let out = String::from_utf8_lossy(&output.stdout);
    let mut rx = 0u64;
    let mut tx = 0u64;
    for line in out.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 10 {
            // netstat -ib format: Name Mtu Network Address Ipkts Ierrs Opkts Oerrs Coll
            rx += parts.get(4).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            tx += parts.get(6).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        }
    }
    Some((rx, tx))
}

#[cfg(target_os = "windows")]
async fn read_net_rx_impl() -> Option<f64> { Some(0.0) }
#[cfg(target_os = "windows")]
async fn read_net_tx_impl() -> Option<f64> { Some(0.0) }

// ── 自动检测本机对外 IP ──
//
// 当 `sip.ip = "0.0.0.0"` (即"绑定所有接口"占位符) 时，前端 dialog
// 不能把这个占位符呈现给用户——它对设备端无意义。我们用 RFC 6724 标准
// 做法：通过 UDP socket 假装连接外部地址，让 OS 选一条路由，再用
// `getsockname` 读 socket 自己的 IP。这避免了解析 `ifconfig`/`ip`
// 输出，跨 Linux/macOS 行为一致。
//
// 返回的 IP 缓存 5 分钟以避免反复系统调用。
fn detect_outbound_ip() -> Option<IpAddr> {
    use std::net::UdpSocket;
    // 8.8.8.8:80 是公网地址；UDP "connect" 不会真发包，但能让 OS
    // 选一条 IPv4 出接口路由。
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let local: SocketAddr = socket.local_addr().ok()?;
    Some(local.ip())
}

/// Like `detect_outbound_ip` but returns only the first non-loopback IPv4
/// from the system's interface list, in case UDP-probe fails (e.g. no
/// outbound route available).  Uses `ifconfig` on macOS, `/proc/net/fib_trie`
/// on Linux.  Best-effort; returns None if no candidate found.
fn detect_ipv4_via_ifconfig() -> Option<IpAddr> {
    // macOS / Linux: parse `ifconfig` output.  Cheap heuristic.
    let output = std::process::Command::new("ifconfig").output().ok()?;
    if !output.status.success() { return None; }
    let out = String::from_utf8_lossy(&output.stdout);
    let mut current_name = String::new();
    let mut current_ips: Vec<Ipv4Addr> = Vec::new();
    let mut candidates: Vec<(String, Ipv4Addr)> = Vec::new();
    for line in out.lines() {
        // macOS: "en0: flags=8863<UP,BROADCAST,SMART,RUNNING,SIMPLEX,MULTICAST> mtu 1500"
        // Linux: "enp0s3: flags=4163<UP,BROADCAST,RUNNING,MULTICAST>  mtu 1500"
        if !line.starts_with(' ') && line.contains(':') && !line.contains("inet ") {
            // Flush previous interface
            if !current_ips.is_empty() && !is_virtual_interface(&current_name) {
                for ip in current_ips.drain(..) {
                    candidates.push((current_name.clone(), ip));
                }
            }
            current_name = line.split(':').next().unwrap_or("").trim().to_string();
            current_ips.clear();
        } else if line.contains("inet ") && !line.contains("inet6") {
            // "    inet 192.168.1.5 netmask 0xffffff00 broadcast 192.168.1.255"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(pos) = parts.iter().position(|&p| p == "inet") {
                if let Some(ip_str) = parts.get(pos + 1) {
                    if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                        if !ip.is_loopback() {
                            current_ips.push(ip);
                        }
                    }
                }
            }
        }
    }
    // Flush last interface
    if !current_ips.is_empty() && !is_virtual_interface(&current_name) {
        for ip in current_ips.drain(..) {
            candidates.push((current_name.clone(), ip));
        }
    }
    candidates.into_iter().next().map(|(_, ip)| IpAddr::V4(ip))
}

fn is_virtual_interface(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Loopback, virtual bridges, VPN tunnels, AWDL, etc.
    n == "lo0" || n == "lo"
        || n.starts_with("awdl")
        || n.starts_with("llw")
        || n.starts_with("utun")
        || n.starts_with("ipsec")
        || n.starts_with("bridge")
        || n.starts_with("vboxnet")
        || n.starts_with("vmnet")
        || n.starts_with("docker")
        || n.starts_with("br-")
        || n.starts_with("veth")
}

fn detect_outbound_ip_cached() -> Option<IpAddr> {
    use std::sync::Mutex;
    use std::time::Instant;
    static CACHE: OnceLock<Mutex<Option<(Instant, IpAddr)>>> = OnceLock::new();
    let m = CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = m.lock().unwrap();
    if let Some((when, ip)) = *guard {
        if when.elapsed() < Duration::from_secs(300) {
            return Some(ip);
        }
    }
    let ip = detect_outbound_ip().or_else(detect_ipv4_via_ifconfig);
    if let Some(ip) = ip {
        *guard = Some((Instant::now(), ip));
    }
    ip
}

/// GET /api/server/media_server/list
pub async fn media_server_list(State(state): State<AppState>) -> Result<Json<WVPResult<Vec<MediaServer>>>, AppError> {
    let list = list_media_servers(&state.pool).await?;
    Ok(Json(WVPResult::success(list)))
}

/// GET /api/server/media_server/online/list — 与 list 同结构，可过滤在线（当前返回全部）
pub async fn media_server_online_list(State(state): State<AppState>) -> Result<Json<WVPResult<Vec<MediaServer>>>, AppError> {
    let list = list_media_servers(&state.pool).await?;
    Ok(Json(WVPResult::success(list)))
}

/// GET /api/server/media_server/one/:id
pub async fn media_server_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<WVPResult<MediaServer>>, AppError> {
    let one = get_media_server_by_id(&state.pool, &id).await?;
    let one = one.ok_or_else(|| crate::error::AppError::business(crate::error::ErrorCode::Error404, "流媒体不存在"))?;
    Ok(Json(WVPResult::success(one)))
}

/// GET /api/server/system/configInfo
///
/// 返回前端 `configInfo.vue` (国标设备页面"接入信息" dialog) 期望的字段：
/// - `sip.id`       = SIP 设备编号（国标 20 位 ID）
/// - `sip.domain`   = SIP 域（realm）
/// - `sip.showIp`   = 对外暴露的 IP（NAT 场景下应配 sdp_ip / stream_ip）
/// - `sip.port`     = SIP UDP 端口
/// - `sip.password` = SIP 接入密码
/// - `addOn.serverId` = 本节点 server 标识（user_settings.server_id），
///                      兜底取 SIP device_id 末 8 位
///
/// 同时保留原 `enabled/tcpPort/realm/...` 字段供其它页面使用。
pub async fn system_config_info(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    let cfg = &state.config;

    // SIP config: 用前端 dialog 期望的字段命名
    let sip_dialog = if let Some(s) = cfg.sip.as_ref() {
        // 优先级：sdp_ip > stream_ip > ip。
        // 如果 `ip` 是 "0.0.0.0"（"绑定所有网卡"占位符）且没有 sdp_ip/stream_ip
        // 配置，自动检测本机对外 IP。
        let show_ip = s.sdp_ip.clone()
            .or_else(|| s.stream_ip.clone())
            .or_else(|| {
                if s.ip == "0.0.0.0" {
                    detect_outbound_ip_cached().map(|ip| ip.to_string())
                } else {
                    Some(s.ip.clone())
                }
            })
            .unwrap_or_else(|| s.ip.clone());
        json!({
            "id":       s.device_id,
            "domain":   s.realm,
            "showIp":   show_ip,
            "port":     s.port,
            "password": s.password,
            // 兼容旧字段
            "enabled":  s.enabled,
            "ip":       s.ip,
            "tcpPort":  s.tcp_port,
            "deviceId": s.device_id,
            "realm":    s.realm,
            "keepaliveTimeout": s.keepalive_timeout,
            "registerTimeout":  s.register_timeout,
            "charset":  s.charset,
        })
    } else {
        serde_json::Value::Null
    };

    // ZLM config (保留原结构)
    let zlm_json = if let Some(z) = &cfg.zlm {
        let servers: Vec<_> = z.servers.iter().map(|sv| {
            json!({
                "id": sv.id,
                "ip": sv.ip,
                "http_port": sv.http_port,
                "https_port": sv.https_port,
                "enabled": sv.enabled,
            })
        }).collect();
        json!({
            "servers": servers,
            "stream_timeout": z.stream_timeout,
            "hook_enabled": z.hook_enabled,
            "hook_url": z.hook_url,
        })
    } else {
        serde_json::Value::Null
    };

    // Database type from URL
    let db_type = {
        let url = &cfg.database.url;
        if url.starts_with("postgres") { "postgres" } else if url.starts_with("mysql") { "mysql" } else { "unknown" }
    };

    // addOn.serverId: 优先用 user_settings.server_id；兜底取 SIP device_id 末 8 位
    let add_on_server_id = cfg.user_settings.as_ref()
        .and_then(|u| u.server_id.clone())
        .or_else(|| {
            cfg.sip.as_ref().map(|s| {
                let id = &s.device_id;
                if id.len() >= 8 { id[id.len() - 8..].to_string() } else { id.clone() }
            })
        })
        .unwrap_or_else(|| "gbserver".to_string());

    let data = json!({
        "sip": sip_dialog,
        "zlm": zlm_json,
        "database": {"type": db_type},
        "version": env!("CARGO_PKG_VERSION"),
        "build": env!("CARGO_PKG_NAME"),
        "addOn": {"serverId": add_on_server_id},
    });
    Json(WVPResult::success(data))
}

// ── 控制台图表 ring buffer ──
//
// 前端 v-charts 是"覆盖式" setData（rows = 新数组），如果后端每次只返回当前
// 一个点，曲线永远只有一个点，看不出"实时变化"。因此在内存里维护最近
// WINDOW_SIZE 个采样点的 ring buffer：每次请求时 push 当前点 + 截断，然后
// 返回 buffer 全部内容。这样前端每次 setData 都会拿到一个有历史轨迹的
// 数组，覆盖式渲染也能画出曲线。
const WINDOW_SIZE: usize = 30; // 30 × 2s = 60s 的滚动窗口

struct SampleBuffers {
    cpu: Mutex<VecDeque<(String, f64)>>,  // (time, fraction 0.0-1.0)
    mem: Mutex<VecDeque<(String, f64)>>,
    net: Mutex<VecDeque<(String, f64, f64)>>,  // (time, out_mbps, in_mbps)
}

static BUFFERS: OnceLock<SampleBuffers> = OnceLock::new();

fn buffers() -> &'static SampleBuffers {
    BUFFERS.get_or_init(|| SampleBuffers {
        cpu: Mutex::new(VecDeque::with_capacity(WINDOW_SIZE + 1)),
        mem: Mutex::new(VecDeque::with_capacity(WINDOW_SIZE + 1)),
        net: Mutex::new(VecDeque::with_capacity(WINDOW_SIZE + 1)),
    })
}

fn push_truncated<T>(dq: &mut VecDeque<T>, item: T) {
    if dq.len() >= WINDOW_SIZE {
        dq.pop_front();
    }
    dq.push_back(item);
}

/// GET /api/server/system/info
///
/// 返回 dashboard 控制台六个 v-charts 组件期望的 rows 结构：
/// - `cpu` / `mem`：`[{time, data}]`，`data` 为 0.0-1.0 的小数
///   （前端 `ConsoleCPU.vue` / `ConsoleMEM.vue` 的 yAxis.max=1，
///    tooltip formatter 用 `v * 100 + '%'`，所以后端不要给百分数）
/// - `disk`：`[{path, free, use}]`，单位 GB
/// - `net`：`[{time, out, in}]`，单位 Mbps（列顺序必须与前端 columns 一致）
/// - `netTotal`：`number`，是 `out`/`in` 峰值向上取整（前端直接赋给 yAxis.max）
pub async fn system_info(State(_state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // CPU: 0.0-1.0 fraction (not 0-100)
    let cpu_usage_pct = read_cpu_usage().await.unwrap_or(0.0);
    let cpu_fraction = cpu_usage_pct / 100.0;

    // Memory: 0.0-1.0 fraction
    let mem_pct = if let Some((total, _avail, used)) = read_memory_info() {
        if total > 0 { (used as f64 / total as f64) * 100.0 } else { 50.0 }
    } else { 50.0 };
    let mem_fraction = mem_pct / 100.0;

    // Disk: list of [{path, free, use}] in GB, one row per real mount point.
    // dashboard 期望多根柱子（每根挂载点一个），所以不写入 ring buffer——
    // 直接返回当前所有挂载点即可（磁盘容量不按秒变化）。
    let disks = read_all_disk_usage();
    let disk_data: Vec<serde_json::Value> = if disks.is_empty() {
        vec![serde_json::json!({
            "path": "/",
            "free": 50.0_f64,
            "use":  50.0_f64,
        })]
    } else {
        disks.iter().map(|(path, total, used)| {
            let total_gb = *total as f64 / (1024.0 * 1024.0 * 1024.0);
            let used_gb  = *used  as f64 / (1024.0 * 1024.0 * 1024.0);
            let free_gb  = (total_gb - used_gb).max(0.0);
            serde_json::json!({
                "path": path,
                "free": free_gb,
                "use":  used_gb,
            })
        }).collect()
    };
    // disk_usage: report the highest usage% across all mounts for the
    // backward-compat scalar field.
    let disk_pct = disks.iter().filter_map(|(_, total, used)| {
        if *total > 0 { Some((*used as f64 / *total as f64) * 100.0) } else { None }
    }).fold(0.0_f64, f64::max);

    // Network I/O: out=upload (tx), in=download (rx)
    let net_rx_mbps = read_network_rx_mbps().await.unwrap_or(0.0);
    let net_tx_mbps = read_network_tx_mbps().await.unwrap_or(0.0);

    // Push current sample into ring buffers and snapshot full window.
    // 这样前端 setData 用整个数组覆盖，就能画出滚动曲线。
    let bufs = buffers();
    push_truncated(&mut bufs.cpu.lock().unwrap(), (now.clone(), cpu_fraction));
    push_truncated(&mut bufs.mem.lock().unwrap(), (now.clone(), mem_fraction));
    push_truncated(&mut bufs.net.lock().unwrap(), (now.clone(), net_tx_mbps, net_rx_mbps));

    let cpu_data: Vec<serde_json::Value> = bufs.cpu.lock().unwrap().iter()
        .map(|(t, v)| serde_json::json!({"time": t, "data": v}))
        .collect();
    let mem_data: Vec<serde_json::Value> = bufs.mem.lock().unwrap().iter()
        .map(|(t, v)| serde_json::json!({"time": t, "data": v}))
        .collect();
    let net_data: Vec<serde_json::Value> = bufs.net.lock().unwrap().iter()
        .map(|(t, out, in_)| serde_json::json!({"time": t, "out": out, "in": in_}))
        .collect();

    // netTotal: yAxis max = peak of CURRENT sample rounded up to next 100 Mbps, min 100
    let peak = net_rx_mbps.max(net_tx_mbps);
    let net_total = if peak < 100.0 {
        100.0
    } else {
        (peak / 100.0).ceil() * 100.0
    };

    let uptime = read_uptime().unwrap_or(3600.0) as u64;

    let data = serde_json::json!({
        "cpu": cpu_data,
        "mem": mem_data,
        "disk": disk_data,
        "net": net_data,
        "netTotal": net_total,
        "uptime": uptime,
        "cpu_usage": cpu_usage_pct,
        "mem_usage": mem_pct,
        "disk_usage": disk_pct,
    });
    Json(WVPResult::success(data))
}

/// GET /api/server/map/config
///
/// 返回前端 `MapComponent.vue` 期望的瓦片源**数组**（每个元素对应
/// 一个瓦片源：`{tilesUrl, coordinateSystem, ...}`）。
///
/// 当前实现：返回空数组 `[]`，让前端走 fallback 路径——用
/// `window.mapParam.tilesUrl` 和 `window.mapParam.coordinateSystem`
/// （由 `web/public/static/js/config.js` 注入）。这样:
///   - `mapConfigList.length === 0` 为 true
///   - 前端 if 分支：`window.mapParam.tilesUrl` 存在 → push 瓦片项
///   - `initMap()` 正确读 `mapTileList[0].tilesUrl`/`coordinateSystem`
///
/// 保留此空数组返回是为了：
///   1. 与前端 `MapComponent.vue` 期望的 array schema 对齐
///   2. 未来 [map] 配置扩展（多瓦片源、代理）时只改 handler 即可
pub async fn map_config(State(_state): State<AppState>) -> Json<WVPResult<Vec<serde_json::Value>>> {
    Json(WVPResult::success(Vec::new()))
}

/// GET /api/server/info
///
/// Returns a *nested* structure that matches the legacy WVP frontend's
/// `v-for="(value, key) in systemInfoList"` in `systemInfo.vue` — the outer
/// object maps a category name (e.g. "服务器") to a sub-object of
/// `key: value` pairs the page renders as a description list.
pub async fn server_info(State(_state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    // Persist a simple start time reference via a static OnceLock
    use std::sync::OnceLock;
    static START_TIME: OnceLock<SystemTime> = OnceLock::new();
    let start = START_TIME.get_or_init(|| SystemTime::now());
    let start_ts = start.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    // Simple uptime calculation from start time
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let uptime = now.saturating_sub(start_ts);
    Json(WVPResult::success(serde_json::json!({
        "服务器": {
            "启动时间": chrono::Local.timestamp_opt(start_ts as i64, 0)
                .single().map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| start_ts.to_string()),
            "运行时长(秒)": uptime,
            "版本": env!("CARGO_PKG_VERSION"),
            "构建": env!("CARGO_PKG_NAME"),
        }
    })))
}

/// GET /api/server/resource/info
///
/// 返回 dashboard `ConsoleResource.vue` 期望的嵌套对象：
///   {
///     device:  {total, online},
///     channel: {total, online},
///     push:    {total, online},   // 推流总数 / 在线推流数
///     proxy:   {total, online},   // 拉流代理总数 / 在线的拉流代理
///   }
pub async fn resource_info(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    // Device / channel counts from DB
    let total_devices = db::count_devices(&state.pool, None, None).await.unwrap_or(0);
    let online_devices = db::count_devices(&state.pool, None, Some(true)).await.unwrap_or(0);
    let total_channels = db::count_all_channels(&state.pool).await.unwrap_or(0);
    let online_channels = db::count_online_channels(&state.pool).await.unwrap_or(0);

    // Stream push / proxy counts
    let push_total = db::stream_push::count_all(&state.pool, None, None).await.unwrap_or(0);
    let push_online = db::stream_push::count_all(&state.pool, None, Some(true)).await.unwrap_or(0);
    let proxy_total = db::stream_proxy::count_all(&state.pool, None, None).await.unwrap_or(0);
    let proxy_online = db::stream_proxy::count_all(&state.pool, None, Some(true)).await.unwrap_or(0);

    let data = serde_json::json!({
        "device":  {"total": total_devices,  "online": online_devices},
        "channel": {"total": total_channels, "online": online_channels},
        "push":    {"total": push_total,     "online": push_online},
        "proxy":   {"total": proxy_total,    "online": proxy_online},
    });
    Json(WVPResult::success(data))
}

// ---------- 占位：前端调用避免 404 ----------
/// GET /api/server/media_server/check
#[derive(Debug, Deserialize)]
pub struct MediaServerCheckQuery {
    pub ip: Option<String>,
    #[serde(alias = "httpPort")]
    pub port: Option<i32>,
    pub secret: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
}

pub async fn media_server_check(
    State(state): State<AppState>,
    Query(q): Query<MediaServerCheckQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let ip = q.ip.unwrap_or_else(|| "127.0.0.1".to_string());
    let http_port = q.port.unwrap_or(80);
    let secret = q.secret.unwrap_or_default();
    let type_ = q.type_.unwrap_or_else(|| "zlm".to_string());
    let temp_client = crate::zlm::ZlmClient::new(&ip, http_port as u16, &secret);

    let mut payload = serde_json::json!({
        "ip": ip,
        "httpPort": http_port,
        "secret": secret,
        "type": type_,
        "autoConfig": true,
        "rtpEnable": false,
        "rtpProxyPort": 30000,
        "rtpPortRange": "30000,30100",
        "sendRtpPortRange": "50000,60000"
    });

    if let Ok(configs) = temp_client.get_server_config().await {
        if let Some(obj) = payload.as_object_mut() {
            let get_i32 = |key: &str| configs.get(key).and_then(|v| i32::from_str(v).ok());
            let get_bool = |key: &str| {
                configs
                    .get(key)
                    .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE"))
            };
            obj.insert(
                "hookIp".to_string(),
                json!(configs.get("hook.hookIp").cloned().unwrap_or_default()),
            );
            obj.insert(
                "sdpIp".to_string(),
                json!(configs.get("rtp_proxy.sdp_ip").cloned().unwrap_or_default()),
            );
            obj.insert(
                "streamIp".to_string(),
                json!(configs.get("general.streamNoneReaderDelayMS").cloned().unwrap_or_default()),
            );
            obj.insert("httpSSlPort".to_string(), json!(get_i32("http.sslport").unwrap_or(443)));
            obj.insert("rtmpPort".to_string(), json!(get_i32("rtmp.port").unwrap_or(1935)));
            obj.insert("rtmpSSlPort".to_string(), json!(get_i32("rtmp.sslport").unwrap_or(0)));
            obj.insert("rtspPort".to_string(), json!(get_i32("rtsp.port").unwrap_or(554)));
            obj.insert("rtspSSLPort".to_string(), json!(get_i32("rtsp.sslport").unwrap_or(0)));
            obj.insert(
                "recordAssistPort".to_string(),
                json!(get_i32("record.port").unwrap_or(0)),
            );
            obj.insert(
                "rtpEnable".to_string(),
                json!(get_bool("rtp_proxy.port_range").unwrap_or(false)),
            );
        }
    }

    if let Some(obj) = payload.as_object_mut() {
        if obj
            .get("streamIp")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .is_empty()
        {
            obj.insert(
                "streamIp".to_string(),
                json!(
                    state
                        .config
                        .zlm
                        .as_ref()
                        .and_then(|cfg| cfg.servers.first().map(|sv| sv.ip.clone()))
                        .unwrap_or_else(|| "127.0.0.1".to_string())
                ),
            );
        }
    }

    Json(WVPResult::success(payload))
}

/// GET /api/server/media_server/record/check
#[derive(Debug, Deserialize)]
pub struct MediaServerRecordCheckQuery {
    pub ip: Option<String>,
    pub port: Option<i32>,
}

pub async fn media_server_record_check(
    State(state): State<AppState>,
    Query(q): Query<MediaServerRecordCheckQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let reachable = state
        .zlm_client
        .as_ref()
        .map(|_| true)
        .unwrap_or(false);
    Json(WVPResult::success(serde_json::json!({
        "success": reachable,
        "ip": q.ip,
        "port": q.port,
        "recordAssistPort": q.port.unwrap_or(0),
    })))
}

/// POST /api/server/media_server/save - 添加或更新媒体服务器
#[derive(Debug, Deserialize)]
pub struct MediaServerSaveBody {
    pub id: Option<String>,
    pub ip: Option<String>,
    #[serde(alias = "hookIp")]
    pub hook_ip: Option<String>,
    #[serde(alias = "sdpIp")]
    pub sdp_ip: Option<String>,
    #[serde(alias = "streamIp")]
    pub stream_ip: Option<String>,
    #[serde(alias = "httpPort")]
    pub http_port: Option<i32>,
    #[serde(alias = "httpSSlPort")]
    pub http_ssl_port: Option<i32>,
    pub secret: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    #[serde(alias = "autoConfig")]
    pub auto_config: Option<bool>,
    #[serde(alias = "rtmpPort")]
    pub rtmp_port: Option<i32>,
    #[serde(alias = "rtmpSSlPort")]
    pub rtmp_ssl_port: Option<i32>,
    #[serde(alias = "rtspPort")]
    pub rtsp_port: Option<i32>,
    #[serde(alias = "rtspSSLPort")]
    pub rtsp_ssl_port: Option<i32>,
    #[serde(alias = "rtpEnable")]
    pub rtp_enable: Option<bool>,
    #[serde(alias = "rtpPortRange")]
    pub rtp_port_range: Option<String>,
    #[serde(alias = "sendRtpPortRange")]
    pub send_rtp_port_range: Option<String>,
    #[serde(alias = "rtpProxyPort")]
    pub rtp_proxy_port: Option<i32>,
    #[serde(alias = "recordAssistPort")]
    pub record_assist_port: Option<i32>,
    #[serde(alias = "defaultServer")]
    pub default_server: Option<bool>,
}

pub async fn media_server_save(
    State(state): State<AppState>,
    Json(body): Json<MediaServerSaveBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or_else(|| format!("media_server_{}", chrono::Utc::now().timestamp_millis()));
    let ip = body.ip.unwrap_or_else(|| "127.0.0.1".to_string());
    let http_port = body.http_port.unwrap_or(8080);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
    // 检查是否已存在
    let existing = get_media_server_by_id(&state.pool, &id).await?;
    
    if existing.is_some() {
        media_server::update(
            &state.pool,
            &id,
            Some(&ip),
            body.hook_ip.as_deref(),
            Some(http_port),
            &now,
        ).await?;
    } else {
        // 添加
        media_server::add(
            &state.pool,
            &id,
            &ip,
            http_port,
            &now,
        ).await?;
    }

    #[cfg(feature = "postgres")]
    sqlx::query(
        r#"UPDATE gb_media_server SET
           hook_ip = COALESCE($1, hook_ip),
           sdp_ip = COALESCE($2, sdp_ip),
           stream_ip = COALESCE($3, stream_ip),
           http_ssl_port = COALESCE($4, http_ssl_port),
           secret = COALESCE($5, secret),
           type = COALESCE($6, type),
           auto_config = COALESCE($7, auto_config),
           rtmp_port = COALESCE($8, rtmp_port),
           rtmp_ssl_port = COALESCE($9, rtmp_ssl_port),
           rtsp_port = COALESCE($10, rtsp_port),
           rtsp_ssl_port = COALESCE($11, rtsp_ssl_port),
           rtp_enable = COALESCE($12, rtp_enable),
           rtp_port_range = COALESCE($13, rtp_port_range),
           send_rtp_port_range = COALESCE($14, send_rtp_port_range),
           rtp_proxy_port = COALESCE($15, rtp_proxy_port),
           record_assist_port = COALESCE($16, record_assist_port),
           default_server = COALESCE($17, default_server),
           update_time = $18
           WHERE id = $19"#,
    )
    .bind(body.hook_ip.as_deref())
    .bind(body.sdp_ip.as_deref())
    .bind(body.stream_ip.as_deref())
    .bind(body.http_ssl_port)
    .bind(body.secret.as_deref())
    .bind(body.type_.as_deref())
    .bind(body.auto_config)
    .bind(body.rtmp_port)
    .bind(body.rtmp_ssl_port)
    .bind(body.rtsp_port)
    .bind(body.rtsp_ssl_port)
    .bind(body.rtp_enable)
    .bind(body.rtp_port_range.as_deref())
    .bind(body.send_rtp_port_range.as_deref())
    .bind(body.rtp_proxy_port)
    .bind(body.record_assist_port)
    .bind(body.default_server)
    .bind(&now)
    .bind(&id)
    .execute(&state.pool)
    .await?;
    #[cfg(feature = "mysql")]
    sqlx::query(
        r#"UPDATE gb_media_server SET
           hook_ip = COALESCE(?, hook_ip),
           sdp_ip = COALESCE(?, sdp_ip),
           stream_ip = COALESCE(?, stream_ip),
           http_ssl_port = COALESCE(?, http_ssl_port),
           secret = COALESCE(?, secret),
           type = COALESCE(?, type),
           auto_config = COALESCE(?, auto_config),
           rtmp_port = COALESCE(?, rtmp_port),
           rtmp_ssl_port = COALESCE(?, rtmp_ssl_port),
           rtsp_port = COALESCE(?, rtsp_port),
           rtsp_ssl_port = COALESCE(?, rtsp_ssl_port),
           rtp_enable = COALESCE(?, rtp_enable),
           rtp_port_range = COALESCE(?, rtp_port_range),
           send_rtp_port_range = COALESCE(?, send_rtp_port_range),
           rtp_proxy_port = COALESCE(?, rtp_proxy_port),
           record_assist_port = COALESCE(?, record_assist_port),
           default_server = COALESCE(?, default_server),
           update_time = ?
           WHERE id = ?"#,
    )
    .bind(body.hook_ip.as_deref())
    .bind(body.sdp_ip.as_deref())
    .bind(body.stream_ip.as_deref())
    .bind(body.http_ssl_port)
    .bind(body.secret.as_deref())
    .bind(body.type_.as_deref())
    .bind(body.auto_config)
    .bind(body.rtmp_port)
    .bind(body.rtmp_ssl_port)
    .bind(body.rtsp_port)
    .bind(body.rtsp_ssl_port)
    .bind(body.rtp_enable)
    .bind(body.rtp_port_range.as_deref())
    .bind(body.send_rtp_port_range.as_deref())
    .bind(body.rtp_proxy_port)
    .bind(body.record_assist_port)
    .bind(body.default_server)
    .bind(&now)
    .bind(&id)
    .execute(&state.pool)
    .await?;

    let auto_config = body.auto_config.unwrap_or(true);
    let mut zlm_hook_errors = Vec::new();
    if auto_config {
        let client = state
            .get_zlm_client(Some(&id))
            .unwrap_or_else(|| std::sync::Arc::new(crate::zlm::ZlmClient::new(
                &ip,
                http_port as u16,
                body.secret.as_deref().unwrap_or_default(),
            )));
        zlm_hook_errors = configure_zlm_hooks(&state, &id, &client).await;
    }
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "autoConfig": auto_config,
        "hookConfigured": auto_config && zlm_hook_errors.is_empty(),
        "hookErrors": zlm_hook_errors,
        "message": "保存成功"
    }))))
}

/// DELETE /api/server/media_server/delete
#[derive(Debug, Deserialize)]
pub struct MediaServerDeleteQuery {
    pub id: Option<String>,
}

pub async fn media_server_delete(
    State(state): State<AppState>,
    Query(q): Query<MediaServerDeleteQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = q
        .id
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();
    if id.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 id 参数"));
    }
    media_server::delete_by_id(&state.pool, &id).await?;
    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "message": "删除成功"
    }))))
}

/// GET /api/server/media_server/media_info
#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct MediaInfoQuery {
    pub app: Option<String>,
    pub stream: Option<String>,
    pub mediaServerId: Option<String>,
}

pub async fn media_server_media_info(
    State(state): State<AppState>,
    Query(q): Query<MediaInfoQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let app = q.app.as_deref().unwrap_or("");
    let stream = q.stream.as_deref().unwrap_or("");
    if app.is_empty() || stream.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 app 或 stream 参数"));
    }

    // 选择 ZLM 客户端
    let zlm_client = if let Some(ref ms_id) = q.mediaServerId {
        state.get_zlm_client(Some(ms_id)).clone()
    } else {
        state.get_zlm_client(None)
    };

    let client = zlm_client.ok_or_else(|| {
        AppError::business(ErrorCode::Error404, "未配置 ZLM 客户端或媒体服务器不存在")
    })?;

    // 常见默认参数：rtmp, __defaultVhost__
    let schema = "rtmp";
    let vhost = "__defaultVhost__";

    match client.get_media_info(schema, vhost, app, stream).await {
        Ok(Some(info)) => {
            let value = serde_json::to_value(info).unwrap_or(serde_json::Value::Null);
            Ok(Json(WVPResult::success(value)))
        }
        Ok(None) => Ok(Json(WVPResult::success(serde_json::Value::Null))),
        Err(e) => Err(AppError::business(ErrorCode::Error500, format!("ZLM 请求失败: {}", e))),
    }
}

/// GET /api/server/media_server/load
///
/// 返回 dashboard `ConsoleNodeLoad.vue` 期望的数组：
///   [
///     {id, push, proxy, gbReceive, gbSend},
///     ...
///   ]
/// - `push`    : 当前在线的推流数（按 media_server_id 过滤，pushing=true）
/// - `proxy`   : 当前在线的拉流代理数（同上）
/// - `gbReceive`: 国标收流数（从 ZLM getServerStats 中取常见键，找不到为 0）
/// - `gbSend`   : 国标推流数（同上）
pub async fn media_server_load(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    let mut server_loads = Vec::new();
    for server_id in state.list_zlm_servers() {
        if let Some(zlm) = state.get_zlm_client(Some(&server_id)) {
            // Push / proxy counts (per media server, active only)
            let push = db::stream_push::count_all(&state.pool, Some(&server_id), Some(true))
                .await.unwrap_or(0);
            let proxy = db::stream_proxy::count_all(&state.pool, Some(&server_id), Some(true))
                .await.unwrap_or(0);

            // gbReceive / gbSend: pull from ZLM stats if exposed, else 0.
            // ZLM getServerStats reports aggregate stream counts; we look for
            // a few well-known keys, falling back to 0 if ZLM doesn't report.
            let stats_map = zlm.get_server_stats().await.unwrap_or_default();
            let pick_i64 = |keys: &[&str]| -> i64 {
                for k in keys {
                    if let Some(v) = stats_map.get(*k) {
                        if let Some(n) = v.as_i64() {
                            return n;
                        }
                        if let Some(s) = v.as_str() {
                            if let Ok(n) = s.parse::<i64>() {
                                return n;
                            }
                        }
                    }
                }
                0
            };
            let gb_receive = pick_i64(&[
                "MediaStreamCount",
                "mediaStreamCount",
                "streamCount",
            ]);
            let gb_send = pick_i64(&[
                "MediaSenderCount",
                "mediaSenderCount",
                "sendRtpCount",
            ]);

            server_loads.push(serde_json::json!({
                "id": server_id,
                "push": push,
                "proxy": proxy,
                "gbReceive": gb_receive,
                "gbSend": gb_send,
            }));
        }
    }
    // Return array directly for frontend
    Json(WVPResult::success(serde_json::Value::Array(server_loads)))
}

/// GET /api/server/map/model-icon/list
pub async fn map_model_icon_list() -> Json<WVPResult<Vec<serde_json::Value>>> {
    Json(WVPResult::success(vec![
        serde_json::json!({
            "id": "camera",
            "name": "标准枪机",
            "icon": "el-icon-video-camera"
        }),
        serde_json::json!({
            "id": "ptz",
            "name": "云台球机",
            "icon": "el-icon-camera"
        }),
        serde_json::json!({
            "id": "vehicle",
            "name": "车载终端",
            "icon": "el-icon-truck"
        }),
    ]))
}

/// Phase 4.5: 统一流视图 —— 一次性聚合 `gb_stream_push` + `gb_stream_proxy` 两表，
/// 通过 `StreamState` trait 屏蔽表差异，返回统一 JSON。
///
/// GET /api/server/stream/all
pub async fn list_all_streams(
    State(state): State<AppState>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let mut unified: Vec<serde_json::Value> = Vec::new();

    // 1) 推流表
    match stream_push::list_paged(&state.pool, 1, 200, None, None).await {
        Ok(pushes) => {
            for s in pushes {
                unified.push(stream_state_to_json("push", &s));
            }
        }
        Err(e) => {
            tracing::warn!("list_all_streams: stream_push query failed: {}", e);
        }
    }

    // 2) 代理拉流表
    match stream_proxy::list_paged(&state.pool, 1, 200, None, None).await {
        Ok(proxies) => {
            for s in proxies {
                unified.push(stream_state_to_json("proxy", &s));
            }
        }
        Err(e) => {
            tracing::warn!("list_all_streams: stream_proxy query failed: {}", e);
        }
    }

    // TODO(phase-5): unify SendRtp streams when table lands.
    // 设计文档 §7.4 要求本接口同时返回 GB / push / proxy / SendRtp 四类流。
    // 目前 Phase 4 仅落地 push + proxy 两类；`src/db/send_rtp.rs` 与
    // `gb_send_rtp` 表均尚未在三个 init SQL 中建表，因此本阶段无法实现
    // `StreamState` impl。等 Phase 5 SendRtp 表 schema 落地后，再追加
    // `SendRtpRecord: StreamState` 并在此处调用 `send_rtp::list_paged` 后
    // 通过 `stream_state_to_json("send_rtp", &rec)` 加入 unified。
    // 详见 Phase 5 任务清单。

    // 3) 汇总统计
    let active_count = unified
        .iter()
        .filter(|v| {
            v.get("is_active")
                .and_then(|x| x.as_bool())
                .unwrap_or(false)
        })
        .count();

    Ok(Json(WVPResult::success(serde_json::json!({
        "total": unified.len(),
        "active": active_count,
        "items": unified,
    }))))
}

/// 通用辅助：把任何 `StreamState` 实现序列化为统一 JSON。
fn stream_state_to_json(kind: &str, s: &dyn StreamState) -> serde_json::Value {
    serde_json::json!({
        "kind": kind,
        "stream_id": s.stream_id(),
        "app": s.app(),
        "status": s.status().as_str(),
        "is_active": s.status().is_active(),
        "media_server_id": s.media_server_id(),
        "device_id": s.device_id(),
        "channel_id": s.channel_id(),
    })
}
