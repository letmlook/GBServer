use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
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
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            timers: TimerValues::default(),
        }
    }
    
    pub fn with_timers(timers: TimerValues) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            timers,
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
                tracing::debug!("Transaction {} retransmit count: {}", id, txn.retransmit_count);
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
