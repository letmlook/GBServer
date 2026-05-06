use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubscriptionType {
    Catalog,
    MobilePosition,
    Alarm,
    Keepalive,
}

impl SubscriptionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SubscriptionType::Catalog => "Catalog",
            SubscriptionType::MobilePosition => "MobilePosition",
            SubscriptionType::Alarm => "Alarm",
            SubscriptionType::Keepalive => "Keepalive",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubscriptionEntry {
    pub device_id: String,
    pub sub_type: SubscriptionType,
    pub call_id: String,
    pub expires: u32,
    pub created_at: DateTime<Utc>,
    pub last_renew: DateTime<Utc>,
    pub interval_secs: u32,
    pub active: bool,
}

pub struct SubscriptionManager {
    subscriptions: DashMap<String, SubscriptionEntry>,
    sip_server: Option<Arc<RwLock<crate::sip::SipServer>>>,
    ws_state: Option<Arc<crate::handlers::websocket::WsState>>,
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self {
            subscriptions: DashMap::new(),
            sip_server: None,
            ws_state: None,
        }
    }

    pub fn set_sip_server(&mut self, server: Arc<RwLock<crate::sip::SipServer>>) {
        self.sip_server = Some(server);
    }

    pub fn set_ws_state(&mut self, ws: Arc<crate::handlers::websocket::WsState>) {
        self.ws_state = Some(ws);
    }

    pub fn subscribe(&self, device_id: &str, sub_type: SubscriptionType, call_id: &str, expires: u32, interval_secs: u32) {
        let key = format!("{}_{}", device_id, sub_type.as_str());
        let now = Utc::now();
        self.subscriptions.insert(key, SubscriptionEntry {
            device_id: device_id.to_string(),
            sub_type,
            call_id: call_id.to_string(),
            expires,
            created_at: now,
            last_renew: now,
            interval_secs,
            active: true,
        });
        tracing::info!("Subscription registered: {} {} expires={}s", device_id, sub_type.as_str(), expires);
    }

    pub fn unsubscribe(&self, device_id: &str, sub_type: SubscriptionType) {
        let key = format!("{}_{}", device_id, sub_type.as_str());
        if let Some((_, _entry)) = self.subscriptions.remove(&key) {
            tracing::info!("Subscription removed: {} {}", device_id, sub_type.as_str());
        }
    }

    pub fn renew(&self, device_id: &str, sub_type: SubscriptionType) -> bool {
        let key = format!("{}_{}", device_id, sub_type.as_str());
        if let Some(mut entry) = self.subscriptions.get_mut(&key) {
            entry.last_renew = Utc::now();
            tracing::debug!("Subscription renewed: {} {}", device_id, sub_type.as_str());
            true
        } else {
            false
        }
    }

    pub fn get(&self, device_id: &str, sub_type: SubscriptionType) -> Option<SubscriptionEntry> {
        let key = format!("{}_{}", device_id, sub_type.as_str());
        self.subscriptions.get(&key).map(|e| e.clone())
    }

    pub fn get_expiring_soon(&self, threshold_secs: i64) -> Vec<SubscriptionEntry> {
        let now = Utc::now();
        self.subscriptions.iter()
            .filter(|e| {
                let elapsed = (now - e.last_renew).num_seconds();
                let remaining = e.expires as i64 - elapsed;
                remaining <= threshold_secs && remaining > 0 && e.active
            })
            .map(|e| e.clone())
            .collect()
    }

    pub fn get_all(&self) -> Vec<SubscriptionEntry> {
        self.subscriptions.iter().map(|e| e.clone()).collect()
    }

    pub fn cleanup_expired(&self) -> Vec<String> {
        let now = Utc::now();
        let mut removed = Vec::new();
        self.subscriptions.retain(|key, entry| {
            let elapsed = (now - entry.created_at).num_seconds() as u32;
            if elapsed >= entry.expires {
                removed.push(key.clone());
                return false;
            }
            true
        });
        removed
    }

    pub async fn run_renewal_loop(&self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));

        loop {
            interval.tick().await;

            let expiring = self.get_expiring_soon(30);
            for entry in expiring {
                tracing::info!("Auto-renewing subscription: {} {}", entry.device_id, entry.sub_type.as_str());
                self.renew(&entry.device_id, entry.sub_type);
            }

            let expired = self.cleanup_expired();
            for key in expired {
                tracing::info!("Subscription expired: {}", key);
            }
        }
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_and_get() {
        let manager = SubscriptionManager::new();
        manager.subscribe("dev1", SubscriptionType::Catalog, "call1", 300, 60);
        let entry = manager.get("dev1", SubscriptionType::Catalog).unwrap();
        assert_eq!(entry.device_id, "dev1");
        assert_eq!(entry.expires, 300);
    }

    #[test]
    fn test_unsubscribe() {
        let manager = SubscriptionManager::new();
        manager.subscribe("dev1", SubscriptionType::MobilePosition, "call1", 300, 60);
        manager.unsubscribe("dev1", SubscriptionType::MobilePosition);
        assert!(manager.get("dev1", SubscriptionType::MobilePosition).is_none());
    }

    #[test]
    fn test_renew() {
        let manager = SubscriptionManager::new();
        manager.subscribe("dev1", SubscriptionType::Alarm, "call1", 300, 60);
        assert!(manager.renew("dev1", SubscriptionType::Alarm));
        assert!(!manager.renew("dev2", SubscriptionType::Alarm));
    }
}
