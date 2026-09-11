use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use chrono::{DateTime, Utc, Duration};
use super::message::SipRequest;
use super::method::SipMethod;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Invite,
    NonInvite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteClientState {
    Calling,
    Proceeding,
    Completed,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteServerState {
    Proceeding,
    Completed,
    Confirmed,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonInviteClientState {
    Trying,
    Proceeding,
    Completed,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonInviteServerState {
    Trying,
    Proceeding,
    Completed,
    Terminated,
}

#[derive(Debug, Clone)]
pub enum TransactionState {
    InviteClient(InviteClientState),
    InviteServer(InviteServerState),
    NonInviteClient(NonInviteClientState),
    NonInviteServer(NonInviteServerState),
}

impl TransactionState {
    pub fn is_terminal(&self) -> bool {
        match self {
            Self::InviteClient(s) => matches!(s, InviteClientState::Terminated),
            Self::InviteServer(s) => matches!(s, InviteServerState::Terminated),
            Self::NonInviteClient(s) => matches!(s, NonInviteClientState::Terminated),
            Self::NonInviteServer(s) => matches!(s, NonInviteServerState::Terminated),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub txn_type: TransactionType,
    pub state: TransactionState,
    pub request: SipRequest,
    pub transport: TransportInfo,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub last_response: Option<u16>,
    pub retransmit_count: u32,
    pub timer_values: TimerValues,
    /// 原始出站字节。**逐字重传**用：RFC 3261 §17.1.1.2 要求重传报文与首次发送
    /// 完全一致（尤其 Via branch 不能变），因此不能靠重新序列化 `request` 来生成。
    pub raw_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct TransportInfo {
    pub via: String,
    pub via_branch: String,
    pub from_tag: String,
    pub to_tag: Option<String>,
    pub call_id: String,
    pub cseq: u32,
    pub cseq_method: String,
    pub peer_addr: String,
    pub transport: String,
}

impl TransportInfo {
    pub fn from_request(req: &SipRequest, peer_addr: &str) -> Option<Self> {
        let via = req.header("via")?.clone();
        let from = req.header("from")?.clone();
        let to = req.header("to")?.clone();
        let call_id = req.header("call-id")?.clone();
        let cseq = req.header("cseq")?.clone();
        
        let from_tag = Self::extract_tag(&from)?;
        let to_tag = Self::extract_tag_optional(&to);
        let cseq_parts: Vec<&str> = cseq.split_whitespace().collect();
        let cseq_num: u32 = cseq_parts.first()?.parse().ok()?;
        let cseq_method = cseq_parts.get(1).unwrap_or(&"INVITE").to_string();
        let via_branch = Self::extract_branch(&via)?;
        
        Some(Self {
            via,
            via_branch,
            from_tag,
            to_tag,
            call_id,
            cseq: cseq_num,
            cseq_method,
            peer_addr: peer_addr.to_string(),
            transport: "UDP".to_string(),
        })
    }
    
    fn extract_tag(uri: &str) -> Option<String> {
        for part in uri.split(';') {
            if part.trim().starts_with("tag=") {
                return Some(part.trim_start_matches("tag=").to_string());
            }
        }
        None
    }
    
    fn extract_tag_optional(uri: &str) -> Option<String> {
        Self::extract_tag(uri)
    }
    
    fn extract_branch(via: &str) -> Option<String> {
        for part in via.split(';') {
            if part.trim().starts_with("branch=") {
                return Some(part.trim_start_matches("branch=").to_string());
            }
        }
        None
    }
    
    pub fn is_invite(&self) -> bool {
        self.cseq_method.to_uppercase() == "INVITE"
    }
    
    pub fn key(&self) -> String {
        format!("{}:{}:{}", self.call_id, self.from_tag, self.cseq)
    }
}

#[derive(Debug, Clone)]
pub struct TimerValues {
    pub t1: Duration,
    pub t2: Duration,
    pub t4: Duration,
    pub timer_a: Option<DateTime<Utc>>,
    pub timer_b: Option<DateTime<Utc>>,
    pub timer_c: Option<DateTime<Utc>>,
    pub timer_d: Option<DateTime<Utc>>,
    pub timer_e: Option<DateTime<Utc>>,
    pub timer_f: Option<DateTime<Utc>>,
    pub timer_k: Option<DateTime<Utc>>,
}

impl Default for TimerValues {
    fn default() -> Self {
        Self {
            t1: Duration::milliseconds(500),
            t2: Duration::seconds(4),
            t4: Duration::seconds(5),
            timer_a: None,
            timer_b: None,
            timer_c: None,
            timer_d: None,
            timer_e: None,
            timer_f: None,
            timer_k: None,
        }
    }
}

impl Transaction {
    pub fn new_invite_client(
        request: SipRequest,
        transport: TransportInfo,
        timers: TimerValues,
    ) -> Self {
        Self {
            id: Self::generate_id(&transport),
            txn_type: TransactionType::Invite,
            state: TransactionState::InviteClient(InviteClientState::Calling),
            request,
            transport,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            last_response: None,
            retransmit_count: 0,
            timer_values: timers,
            raw_bytes: None,
        }
    }
    
    pub fn new_invite_server(
        request: SipRequest,
        transport: TransportInfo,
        timers: TimerValues,
    ) -> Self {
        Self {
            id: Self::generate_id(&transport),
            txn_type: TransactionType::Invite,
            state: TransactionState::InviteServer(InviteServerState::Proceeding),
            request,
            transport,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            last_response: None,
            retransmit_count: 0,
            timer_values: timers,
            raw_bytes: None,
        }
    }
    
    pub fn new_noninvite_client(
        request: SipRequest,
        transport: TransportInfo,
        timers: TimerValues,
    ) -> Self {
        Self {
            id: Self::generate_id(&transport),
            txn_type: TransactionType::NonInvite,
            state: TransactionState::NonInviteClient(NonInviteClientState::Trying),
            request,
            transport,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            last_response: None,
            retransmit_count: 0,
            timer_values: timers,
            raw_bytes: None,
        }
    }
    
    pub fn new_noninvite_server(
        request: SipRequest,
        transport: TransportInfo,
        timers: TimerValues,
    ) -> Self {
        Self {
            id: Self::generate_id(&transport),
            txn_type: TransactionType::NonInvite,
            state: TransactionState::NonInviteServer(NonInviteServerState::Trying),
            request,
            transport,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            last_response: None,
            retransmit_count: 0,
            timer_values: timers,
            raw_bytes: None,
        }
    }
    
    fn generate_id(transport: &TransportInfo) -> String {
        format!("{}-{}", transport.call_id, transport.via_branch)
    }
    
    pub fn is_invite(&self) -> bool {
        self.txn_type == TransactionType::Invite
    }
    
    pub fn handle_response(&mut self, status_code: u16) {
        self.last_response = Some(status_code);
        self.last_activity = Utc::now();
        
        match (&mut self.state, status_code) {
            (TransactionState::InviteClient(s), 100..=199) => {
                *s = InviteClientState::Proceeding;
            }
            (TransactionState::InviteClient(s), 200..=699) => {
                *s = InviteClientState::Completed;
            }
            (TransactionState::InviteServer(s), _) if self.transport.is_invite() => {
                *s = InviteServerState::Completed;
            }
            (TransactionState::NonInviteClient(s), 200..=299) => {
                *s = NonInviteClientState::Terminated;
            }
            (TransactionState::NonInviteClient(s), 300..=699) => {
                *s = NonInviteClientState::Completed;
            }
            (TransactionState::NonInviteServer(s), _) => {
                *s = NonInviteServerState::Completed;
            }
            _ => {}
        }
    }
    
    pub fn handle_ack(&mut self) {
        if let TransactionState::InviteServer(s) = &mut self.state {
            if matches!(s, InviteServerState::Completed) {
                *s = InviteServerState::Confirmed;
            }
        }
    }
    
    pub fn handle_timeout(&mut self, timer_name: &str) {
        match timer_name {
            "A" => {
                if let TransactionState::InviteClient(InviteClientState::Calling) = &self.state {
                    self.retransmit_count += 1;
                    if self.retransmit_count >= 10 {
                        self.state = TransactionState::InviteClient(InviteClientState::Terminated);
                    }
                }
            }
            "B" => {
                if let TransactionState::InviteClient(InviteClientState::Calling) = &self.state {
                    self.state = TransactionState::InviteClient(InviteClientState::Terminated);
                }
            }
            "D" => {
                if let TransactionState::InviteClient(s) = &self.state {
                    if matches!(s, InviteClientState::Completed) {
                        self.state = TransactionState::InviteClient(InviteClientState::Terminated);
                    }
                }
            }
            "E" => {
                if let TransactionState::NonInviteClient(s) = &self.state {
                    if matches!(s, NonInviteClientState::Proceeding) {
                        self.retransmit_count += 1;
                    }
                }
            }
            "F" => {
                if let TransactionState::NonInviteClient(s) = &self.state {
                    if matches!(s, NonInviteClientState::Trying | NonInviteClientState::Proceeding) {
                        self.state = TransactionState::NonInviteClient(NonInviteClientState::Terminated);
                    }
                }
            }
            "K" => {
                if let TransactionState::NonInviteServer(s) = &self.state {
                    if matches!(s, NonInviteServerState::Completed) {
                        self.state = TransactionState::NonInviteServer(NonInviteServerState::Terminated);
                    }
                }
            }
            _ => {}
        }
        self.last_activity = Utc::now();
    }
    
    pub fn should_cleanup(&self, max_age_secs: i64) -> bool {
        if self.state.is_terminal() {
            let age = (Utc::now() - self.last_activity).num_seconds();
            return age > max_age_secs;
        }
        false
    }
}

pub struct TransactionManager {
    transactions: Arc<RwLock<HashMap<String, Transaction>>>,
    timers: TimerValues,
    /// 出站发送通道 `(目的地址, 原始字节)`。
    ///
    /// 事务层本身不持有 socket；由 `SipServer` 注入该通道后，
    /// `process_timers` 才具备"真正把重传报文发出去"的能力。
    /// 未注入时事务层退化为只记时（与 2026-09-11 之前的行为一致）。
    outbound: Arc<RwLock<Option<mpsc::UnboundedSender<(SocketAddr, Vec<u8>)>>>>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            timers: TimerValues::default(),
            outbound: Arc::new(RwLock::new(None)),
        }
    }
    
    pub fn with_timers(timers: TimerValues) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            timers,
            outbound: Arc::new(RwLock::new(None)),
        }
    }

    /// 注入出站发送通道，使事务层具备真实重传能力。
    pub async fn set_outbound_sink(&self, sink: mpsc::UnboundedSender<(SocketAddr, Vec<u8>)>) {
        *self.outbound.write().await = Some(sink);
    }

    /// 为已登记的事务附加原始出站字节（用于逐字重传）。
    pub async fn attach_raw_bytes(&self, id: &str, raw: Vec<u8>) {
        let mut guard = self.transactions.write().await;
        if let Some(txn) = guard.get_mut(id) {
            txn.raw_bytes = Some(raw);
        }
    }
    
    pub async fn add(&self, txn: Transaction) {
        self.transactions.write().await.insert(txn.id.clone(), txn);
    }
    
    pub async fn get(&self, id: &str) -> Option<Transaction> {
        self.transactions.read().await.get(id).cloned()
    }
    
    pub async fn get_by_call_id(&self, call_id: &str) -> Vec<Transaction> {
        self.transactions.read().await
            .values()
            .filter(|t| t.transport.call_id == call_id)
            .cloned()
            .collect()
    }
    
    pub async fn get_invite_transaction(&self, call_id: &str) -> Option<Transaction> {
        self.transactions.read().await
            .values()
            .find(|t| t.transport.call_id == call_id && t.is_invite())
            .cloned()
    }
    
    pub async fn remove(&self, id: &str) {
        self.transactions.write().await.remove(id);
    }
    
    pub async fn update(&self, txn: &Transaction) {
        let mut guard = self.transactions.write().await;
        guard.insert(txn.id.clone(), txn.clone());
    }
    
    pub async fn handle_request(&self, req: &SipRequest, peer_addr: &str) -> Option<Transaction> {
        let transport = TransportInfo::from_request(req, peer_addr)?;
        let txn = if transport.is_invite() {
            Transaction::new_invite_server(req.clone(), transport, self.timers.clone())
        } else {
            Transaction::new_noninvite_server(req.clone(), transport, self.timers.clone())
        };
        
        self.add(txn.clone()).await;
        Some(txn)
    }
    
    pub async fn handle_response(&self, call_id: &str, cseq: u32, status_code: u16) -> Option<Transaction> {
        let mut guard = self.transactions.write().await;
        
        let txn = guard.values_mut()
            .find(|t| t.transport.call_id == call_id && t.transport.cseq == cseq)?;
        
        txn.handle_response(status_code);
        Some(txn.clone())
    }
    
    pub async fn cleanup_expired(&self, max_age_secs: i64) {
        let mut guard = self.transactions.write().await;
        guard.retain(|_, txn| !txn.should_cleanup(max_age_secs));
    }
    
    pub async fn get_pending_invites(&self) -> Vec<Transaction> {
        self.transactions.read().await
            .values()
            .filter(|t| {
                matches!(t.state, 
                    TransactionState::InviteClient(InviteClientState::Calling) |
                    TransactionState::InviteClient(InviteClientState::Proceeding)
                )
            })
            .cloned()
            .collect()
    }
    
    pub async fn cancel_invite(&self, call_id: &str) -> Option<SipRequest> {
        let txn = self.get_invite_transaction(call_id).await?;
        
        let mut cancel_req = SipRequest::new(SipMethod::Cancel, txn.request.uri.clone());
        cancel_req.set_header("via", &txn.transport.via);
        cancel_req.set_header("from", &format!("<sip:{}>;tag={}", 
            Self::extract_uri_from_via(&txn.transport.via),
            txn.transport.from_tag
        ));
        cancel_req.set_header("to", "");
        cancel_req.set_header("call-id", &txn.transport.call_id);
        cancel_req.set_header("cseq", &format!("{} CANCEL", txn.transport.cseq));
        cancel_req.set_header("max-forwards", "70");
        
        Some(cancel_req)
    }
    
    fn extract_uri_from_via(via: &str) -> String {
        for part in via.split(' ') {
            if part.contains('@') {
                return part.to_string();
            }
        }
        String::new()
    }
    
    pub fn timers(&self) -> &TimerValues {
        &self.timers
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TransactionManager {
    /// 启动事务超时和重传处理任务
    pub fn start_timer_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
            loop {
                interval.tick().await;
                self.process_timers().await;
            }
        })
    }
    
    /// 处理所有事务的定时器
    async fn process_timers(&self) {
        let now = Utc::now();
        let mut to_retransmit = Vec::new();
        let mut to_terminate = Vec::new();
        
        {
            let guard = self.transactions.read().await;
            for (id, txn) in guard.iter() {
                // 检查是否需要重传
                if txn.needs_retransmit(now) {
                    to_retransmit.push(id.clone());
                }
                // 检查是否超时
                if txn.is_timeout(now) {
                    to_terminate.push(id.clone());
                }
            }
        }
        
        // 处理重传
        for id in to_retransmit {
            if let Some(mut txn) = self.get(&id).await {
                txn.retransmit_count += 1;
                txn.last_activity = now;
                self.update(&txn).await;

                // 真正把原始字节重发出去（RFC 3261 §17.1.1.2 / §17.1.2.2）。
                // 缺原始字节或未注入出站通道时无法发送，仅保留计数便于观测 ——
                // 2026-09-11 之前这里**只自增计数并打日志，从不发送**。
                match (
                    txn.raw_bytes.as_ref(),
                    txn.transport.peer_addr.parse::<SocketAddr>(),
                ) {
                    (Some(raw), Ok(addr)) => {
                        let sink = self.outbound.read().await;
                        match sink.as_ref() {
                            Some(tx) => {
                                if tx.send((addr, raw.clone())).is_err() {
                                    tracing::warn!("事务 {} 重传失败：出站通道已关闭", id);
                                } else {
                                    tracing::debug!(
                                        "事务 {} 第 {} 次重传 -> {}",
                                        id,
                                        txn.retransmit_count,
                                        addr
                                    );
                                }
                            }
                            None => {
                                tracing::debug!("事务 {} 需重传但未注入出站通道", id);
                            }
                        }
                    }
                    _ => {
                        tracing::debug!("事务 {} 需重传但缺少原始字节或目的地址", id);
                    }
                }
            }
        }
        
        // 处理超时
        for id in to_terminate {
            if let Some(mut txn) = self.get(&id).await {
                txn.terminate();
                self.update(&txn).await;
                tracing::warn!("Transaction {} timed out", id);
            }
        }
        
        // 清理过期事务
        self.cleanup_expired(300).await;
    }
}

impl Transaction {
    /// 检查是否需要重传
    pub fn needs_retransmit(&self, now: DateTime<Utc>) -> bool {
        if self.state.is_terminal() {
            return false;
        }
        
        // 最大重传次数
        let max_retransmit = if self.is_invite() { 7 } else { 11 };
        if self.retransmit_count >= max_retransmit {
            return false;
        }
        
        // 计算下次重传时间 (指数退避)
        let base_timeout = self.timer_values.t1.num_milliseconds();
        let max_timeout = self.timer_values.t2.num_milliseconds();
        let next_timeout = std::cmp::min(
            base_timeout * (1 << self.retransmit_count),
            max_timeout
        );
        
        let elapsed = (now - self.last_activity).num_milliseconds();
        elapsed >= next_timeout
    }
    
    /// 检查是否超时
    pub fn is_timeout(&self, now: DateTime<Utc>) -> bool {
        if self.state.is_terminal() {
            return false;
        }
        
        // INVITE 事务超时时间: 64*T1 (默认 32秒)
        // 非 INVITE 事务超时时间: 64*T1 (默认 32秒)
        let timeout_ms = 64 * self.timer_values.t1.num_milliseconds();
        let elapsed = (now - self.created_at).num_milliseconds();
        elapsed >= timeout_ms
    }
    
    /// 终止事务
    pub fn terminate(&mut self) {
        self.state = match self.txn_type {
            TransactionType::Invite => {
                if matches!(self.state, TransactionState::InviteClient(_)) {
                    TransactionState::InviteClient(InviteClientState::Terminated)
                } else {
                    TransactionState::InviteServer(InviteServerState::Terminated)
                }
            }
            TransactionType::NonInvite => {
                if matches!(self.state, TransactionState::NonInviteClient(_)) {
                    TransactionState::NonInviteClient(NonInviteClientState::Terminated)
                } else {
                    TransactionState::NonInviteServer(NonInviteServerState::Terminated)
                }
            }
        };
        self.last_activity = Utc::now();
    }
    
    /// 获取需要重传的请求
    pub fn get_retransmit_request(&self) -> Option<&SipRequest> {
        if self.needs_retransmit(Utc::now()) && !self.state.is_terminal() {
            Some(&self.request)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod retransmit_tests {
    use super::*;
    use crate::sip::core::{Parser, SipMessage};
    use tokio::sync::mpsc;

    fn sample_request(call_id: &str) -> SipRequest {
        let raw = format!(
            "MESSAGE sip:34020000001320000001@127.0.0.1:5060 SIP/2.0\r\n\
             Via: SIP/2.0/UDP 127.0.0.1:5060;branch=z9hG4bKtest;rport\r\n\
             From: <sip:34020000002000000001@127.0.0.1:5060>;tag=abc\r\n\
             To: <sip:34020000001320000001@127.0.0.1:5060>\r\n\
             Call-ID: {}\r\n\
             CSeq: 1 MESSAGE\r\n\
             Max-Forwards: 70\r\n\
             Content-Length: 0\r\n\r\n",
            call_id
        );
        match Parser::parse(raw.as_bytes()).expect("parse sample request") {
            SipMessage::Request(r) => r,
            _ => panic!("expected request"),
        }
    }

    fn fast_timers() -> TimerValues {
        TimerValues {
            t1: Duration::milliseconds(20),
            ..Default::default()
        }
    }

    async fn register_txn(
        mgr: &Arc<TransactionManager>,
        call_id: &str,
        raw: Option<Vec<u8>>,
    ) -> String {
        let req = sample_request(call_id);
        let ti = TransportInfo::from_request(&req, "127.0.0.1:5061").expect("transport info");
        let txn = Transaction::new_noninvite_client(req, ti, mgr.timers().clone());
        let id = txn.id.clone();
        mgr.add(txn).await;
        if let Some(bytes) = raw {
            mgr.attach_raw_bytes(&id, bytes).await;
        }
        id
    }

    /// 回归保护：事务层必须**真的把原文重传出去**。
    ///
    /// 2026-09-11 之前 `process_timers` 只自增 `retransmit_count` 并打日志，
    /// 从不发送任何东西 —— 即 RFC 3261 §17 的 UDP 重传完全没生效。
    #[tokio::test]
    async fn test_retransmit_actually_sends_raw_bytes() {
        let mgr = Arc::new(TransactionManager::with_timers(fast_timers()));
        let (tx, mut rx) = mpsc::unbounded_channel::<(SocketAddr, Vec<u8>)>();
        mgr.set_outbound_sink(tx).await;

        let raw = b"RAW-SIP-REQUEST-BYTES".to_vec();
        let _id = register_txn(&mgr, "call-retrans-1", Some(raw.clone())).await;

        let _timer = mgr.clone().start_timer_task();
        let got = tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv())
            .await
            .expect("应在超时前收到重传报文")
            .expect("出站通道不应关闭");

        assert_eq!(got.0, "127.0.0.1:5061".parse::<SocketAddr>().unwrap());
        assert_eq!(
            got.1, raw,
            "重传必须逐字发送原始字节（Via branch 不能变）"
        );
    }

    /// 未附加原始字节时不应发送，也不应 panic（仅保留计数便于观测）。
    #[tokio::test]
    async fn test_no_retransmit_without_raw_bytes() {
        let mgr = Arc::new(TransactionManager::with_timers(fast_timers()));
        let (tx, mut rx) = mpsc::unbounded_channel::<(SocketAddr, Vec<u8>)>();
        mgr.set_outbound_sink(tx).await;

        let _id = register_txn(&mgr, "call-retrans-2", None).await;
        let _timer = mgr.clone().start_timer_task();

        let res = tokio::time::timeout(std::time::Duration::from_millis(300), rx.recv()).await;
        assert!(res.is_err(), "缺少原始字节时不应发出任何报文");
    }

    /// 未注入出站通道时不应 panic（退化为只记时）。
    #[tokio::test]
    async fn test_retransmit_without_sink_does_not_panic() {
        let mgr = Arc::new(TransactionManager::with_timers(fast_timers()));
        let _id = register_txn(&mgr, "call-retrans-3", Some(b"x".to_vec())).await;
        let _timer = mgr.clone().start_timer_task();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    /// 收到响应后必须**停止重传**（否则会向设备发重复请求）。
    #[tokio::test]
    async fn test_handle_response_stops_retransmission() {
        let mgr = Arc::new(TransactionManager::with_timers(fast_timers()));
        let (tx, mut rx) = mpsc::unbounded_channel::<(SocketAddr, Vec<u8>)>();
        mgr.set_outbound_sink(tx).await;

        let id = register_txn(&mgr, "call-retrans-4", Some(b"y".to_vec())).await;

        // 收到 200 终态响应
        let txn = mgr.handle_response("call-retrans-4", 1, 200).await;
        assert!(txn.is_some(), "应能按 call_id + cseq 匹配到事务");
        assert!(
            mgr.get(&id).await.map(|t| t.state.is_terminal()).unwrap_or(true),
            "200 之后事务应进入终态"
        );

        let _timer = mgr.clone().start_timer_task();
        let res = tokio::time::timeout(std::time::Duration::from_millis(300), rx.recv()).await;
        assert!(res.is_err(), "事务已终态，不应再重传");
    }

    /// 重传次数必须按指数退避递增且有上限（RFC 3261 非 INVITE 为 11 次）。
    #[tokio::test]
    async fn test_retransmit_backoff_and_cap() {
        let mgr = Arc::new(TransactionManager::with_timers(fast_timers()));
        let (tx, _rx) = mpsc::unbounded_channel::<(SocketAddr, Vec<u8>)>();
        mgr.set_outbound_sink(tx).await;
        let id = register_txn(&mgr, "call-retrans-5", Some(b"z".to_vec())).await;

        let _timer = mgr.clone().start_timer_task();
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;

        let count = mgr.get(&id).await.map(|t| t.retransmit_count).unwrap_or(0);
        assert!(count >= 1, "应已发生重传，实际 {}", count);
        assert!(count <= 11, "非 INVITE 事务重传上限为 11 次，实际 {}", count);
    }
}
