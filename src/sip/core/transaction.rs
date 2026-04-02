//! SIP 事务管理

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use super::message::SipRequest;

#[derive(Debug, Clone)]
pub enum TransactionType {
    Invite,
    NonInvite,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub msg: SipRequest,
    pub transport: TransportInfo,
    pub created_at: DateTime<Utc>,
    pub last_response: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct TransportInfo {
    pub via: String,
    pub from_tag: String,
    pub to_tag: Option<String>,
    pub call_id: String,
    pub cseq: u32,
    pub method: String,
    pub peer_addr: String,
}

pub struct TransactionManager {
    transactions: Arc<RwLock<HashMap<String, Transaction>>>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add(&self, txn: Transaction) {
        self.transactions.write().await.insert(txn.id.clone(), txn);
    }

    pub async fn get(&self, id: &str) -> Option<Transaction> {
        self.transactions.read().await.get(id).cloned()
    }

    pub async fn remove(&self, id: &str) {
        self.transactions.write().await.remove(id);
    }

    pub async fn cleanup_expired(&self, max_age_secs: i64) {
        let now = Utc::now();
        let mut guard = self.transactions.write().await;
        guard.retain(|_, txn| {
            let age = (now - txn.created_at).num_seconds();
            age < max_age_secs
        });
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}
