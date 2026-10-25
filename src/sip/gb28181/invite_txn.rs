//! 入站 INVITE 的**重传去重**（RFC 3261 §17.2.1）。
//!
//! ## 为什么需要它
//!
//! RFC 3261 §17.2.1 要求：服务端对**重传的同一请求**必须**重发同一份响应**，
//! 而不是重新处理一遍。本平台此前**没有服务端事务层**
//! （`sip/core/transaction.rs` 的 `TransactionManager::handle_request` 是死代码，
//! 全仓库无调用点），于是设备重发 INVITE（没收到 200 OK 时的**正常**行为）或
//! 攻击者直接重放同一条 INVITE 时，平台会**完整地重新处理一遍**：
//!
//! 1. 每次都新开一个 ZLM RTP 收流端口，200 OK 里通告的 `m=` 端口随之改变 ——
//!    而设备仍按**第一次** 200 OK 的端口推流 ⇒ 媒体落到已被覆盖掉的孤儿端口，
//!    表现正是「设备在线但一帧 RTP 都没有」；
//! 2. 同 Call-ID 的旧会话被 `create()` 直接覆盖，旧会话的 `zlm_stream_id`
//!    随之丢失，**没有任何地方再 `close_rtp_server`** ⇒ 收流端口泄漏
//!    （BYE 与超时清理都只作用于当前会话那一个端口）。
//!
//! ## 判据
//!
//! 客户端事务的**重传**必须带**完全相同**的 `Call-ID`、**同一个** `Via`
//! header、**同一个** `CSeq`（RFC 3261 §17.2.1）—— 三者相同即认定是重传。
//!
//! 注意**不能**拿 `From` 当判据：它在 UDP 上可随意伪造。`Via` + `CSeq` +
//! `Call-ID` 的组合是「我们回过的那一条」的特征，攻击者伪造不出我们**回显过**
//! 的那份 Via（平台的应答里 Via 带 `rport` 与 branch，与请求里的不同）。
//!
//! ## 边界
//!
//! * 只缓存**成功应答**（200 OK）的 INVITE —— 只有它带 SDP 与收流端口；
//! * 缓存条目有 TTL（覆盖 RFC 的 32s 重传窗口）与容量上限；
//! * 重传命中时**原样重发**同一份响应报文（含 SDP 与**同一个**端口）。

use dashmap::DashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// RFC 3261 的客户端事务重传窗口是 32s（Timer D）；留一倍余量。
pub const INVITE_TXN_TTL: Duration = Duration::from_secs(64);

/// 缓存条目的容量上限（防止被伪造的 Call-ID 把内存撑爆）。
pub const INVITE_TXN_MAX_ENTRIES: usize = 1024;

/// 一条入站 INVITE 的**重传身份**。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InviteTxnKey {
    /// `Call-ID` 头
    pub call_id: String,
    /// `Via` 头（**请求里的原样**）
    pub via: String,
    /// `CSeq` 头（含方法名）
    pub cseq: String,
}

impl InviteTxnKey {
    pub fn new(call_id: &str, via: &str, cseq: &str) -> Self {
        Self {
            call_id: call_id.to_string(),
            via: via.to_string(),
            cseq: cseq.to_string(),
        }
    }
}

/// 缓存的响应（重传时**原样重发**）。
#[derive(Debug, Clone)]
pub struct CachedInviteResponse {
    /// 完整的响应报文（头 + 正文）
    pub response: String,
    /// 写入时刻
    pub at: Instant,
}

impl CachedInviteResponse {
    pub fn is_fresh(&self, now: Instant) -> bool {
        now.duration_since(self.at) < INVITE_TXN_TTL
    }
}

/// 进程级的入站 INVITE 事务缓存。
pub fn invite_txn_cache() -> &'static DashMap<InviteTxnKey, CachedInviteResponse> {
    static CACHE: OnceLock<DashMap<InviteTxnKey, CachedInviteResponse>> = OnceLock::new();
    CACHE.get_or_init(DashMap::new)
}

/// 查一条**未过期**的缓存响应。
///
/// 过期条目顺手删掉，别让它一直占着。
pub fn get_fresh_response(
    cache: &DashMap<InviteTxnKey, CachedInviteResponse>,
    key: &InviteTxnKey,
) -> Option<String> {
    let now = Instant::now();
    let hit = cache.get(key).map(|v| v.clone());
    match hit {
        Some(v) if v.is_fresh(now) => Some(v.response),
        Some(_) => {
            cache.remove(key);
            None
        }
        None => None,
    }
}

/// 记下一条**成功应答**的 INVITE 响应，供后续重传原样重发。
pub fn remember_response(
    cache: &DashMap<InviteTxnKey, CachedInviteResponse>,
    key: InviteTxnKey,
    response: String,
) {
    // 容量上限：先清过期项，仍超限就按写入顺序丢最旧的
    if cache.len() >= INVITE_TXN_MAX_ENTRIES {
        let now = Instant::now();
        cache.retain(|_, v| v.is_fresh(now));
        while cache.len() >= INVITE_TXN_MAX_ENTRIES {
            let oldest = cache
                .iter()
                .min_by_key(|e| e.value().at)
                .map(|e| e.key().clone());
            match oldest {
                Some(k) => {
                    cache.remove(&k);
                }
                None => break,
            }
        }
    }
    cache.insert(
        key,
        CachedInviteResponse {
            response,
            at: Instant::now(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(call_id: &str) -> InviteTxnKey {
        InviteTxnKey::new(
            call_id,
            "SIP/2.0/UDP 10.0.0.9:5060;branch=z9hG4bKa1",
            "1 INVITE",
        )
    }

    /// **回归**：重传（Call-ID / Via / CSeq 三者相同）必须命中同一条缓存。
    #[test]
    fn retransmission_hits_the_same_entry() {
        let cache: DashMap<InviteTxnKey, CachedInviteResponse> = DashMap::new();
        let k = key("call-1");
        remember_response(&cache, k.clone(), "SIP/2.0 200 OK\r\n...".to_string());

        // 同一条请求重发 → 拿到**同一份**响应（含同一个 SDP / 同一个端口）
        let got = get_fresh_response(&cache, &k).expect("重传必须命中缓存");
        assert!(got.contains("200 OK"));
        assert_eq!(cache.len(), 1, "重传不得新增缓存条目");
    }

    /// **回归**：Call-ID / Via / CSeq 任一不同 ⇒ 是**新请求**，不得命中。
    #[test]
    fn a_new_request_does_not_hit_the_cache() {
        let cache: DashMap<InviteTxnKey, CachedInviteResponse> = DashMap::new();
        remember_response(&cache, key("call-1"), "resp-1".to_string());

        // 不同 Call-ID
        assert!(get_fresh_response(&cache, &key("call-2")).is_none());
        // 不同 Via（不同 branch）
        let other_via = InviteTxnKey::new(
            "call-1",
            "SIP/2.0/UDP 10.0.0.9:5060;branch=z9hG4bKa2",
            "1 INVITE",
        );
        assert!(get_fresh_response(&cache, &other_via).is_none());
        // 不同 CSeq
        let other_cseq = InviteTxnKey::new(
            "call-1",
            "SIP/2.0/UDP 10.0.0.9:5060;branch=z9hG4bKa1",
            "2 INVITE",
        );
        assert!(get_fresh_response(&cache, &other_cseq).is_none());
    }

    /// **回归**：过期条目不得再被重发（RFC 的重传窗口是 32s，TTL 64s）。
    #[test]
    fn expired_entries_are_not_replayed() {
        let cache: DashMap<InviteTxnKey, CachedInviteResponse> = DashMap::new();
        let k = key("call-ttl");
        remember_response(&cache, k.clone(), "resp".to_string());

        // 人为把写入时刻改到 TTL 之前
        cache.insert(
            k.clone(),
            CachedInviteResponse {
                response: "resp".to_string(),
                at: Instant::now() - (INVITE_TXN_TTL + Duration::from_secs(1)),
            },
        );
        assert!(
            get_fresh_response(&cache, &k).is_none(),
            "过期条目不得被重发"
        );
        assert!(cache.get(&k).is_none(), "过期条目应被顺手清掉");
    }

    /// **回归（接线守卫）**：生产路径必须真的用上这套去重。
    ///
    /// 本条缺陷的核心不是「缓存写错了」，而是**生产路径压根没有去重** ——
    /// 单元函数自洽，生产却把每一条重传都当成新请求完整处理一遍。
    /// 这里用文本扫描守住接线（与 `src/dialect_guard.rs` 同一套路）。
    #[test]
    fn production_invite_path_uses_the_dedup_cache() {
        let src = include_str!("../server.rs");
        assert!(
            src.contains("get_fresh_response"),
            "src/sip/server.rs 的 handle_invite 必须查重传缓存 —— 否则重传会重新开一个 RTP 端口"
        );
        assert!(
            src.contains("remember_response"),
            "src/sip/server.rs 的 handle_invite 必须把 200 OK 记进重传缓存"
        );
        assert!(
            src.contains("invite_manager_cleanup.cleanup_expired"),
            "已终止的 INVITE 会话必须接上 cleanup_expired —— 否则它们在内存里持续累积"
        );
        assert!(
            src.contains("close_rtp_server(old_stream)"),
            "覆盖旧会话前必须回收它的 ZLM 收流端口 —— 否则端口泄漏"
        );
    }

    /// **回归**：缓存有容量上限（被伪造的 Call-ID 不能把内存撑爆）。
    #[test]
    fn cache_is_bounded() {
        let cache: DashMap<InviteTxnKey, CachedInviteResponse> = DashMap::new();
        for i in 0..(INVITE_TXN_MAX_ENTRIES + 50) {
            remember_response(&cache, key(&format!("call-{i}")), "resp".to_string());
        }
        assert!(
            cache.len() <= INVITE_TXN_MAX_ENTRIES,
            "缓存容量必须受限，实际 {}",
            cache.len()
        );
    }
}
