use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct CatalogSubscription {
    pub call_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub addr: SocketAddr,
    pub via: String,
    pub from_tag: String,
    pub to_tag: String,
    pub expires: u32,
    pub created_at: DateTime<Utc>,
    pub last_notify: DateTime<Utc>,
}

impl CatalogSubscription {
    pub fn new(
        call_id: &str,
        device_id: &str,
        addr: SocketAddr,
        via: &str,
        from_tag: &str,
        to_tag: &str,
        expires: u32,
    ) -> Self {
        let now = Utc::now();
        Self {
            call_id: call_id.to_string(),
            device_id: device_id.to_string(),
            channel_id: String::new(),
            addr,
            via: via.to_string(),
            from_tag: from_tag.to_string(),
            to_tag: to_tag.to_string(),
            expires,
            created_at: now,
            last_notify: now,
        }
    }

    pub fn is_expired(&self) -> bool {
        let elapsed = (Utc::now() - self.created_at).num_seconds() as u32;
        elapsed >= self.expires
    }
}

pub struct CatalogSubscriptionManager {
    subscriptions: Arc<RwLock<HashMap<String, CatalogSubscription>>>,
}

impl CatalogSubscriptionManager {
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn subscribe(&self, subscription: CatalogSubscription) {
        self.subscriptions.write().await
            .insert(subscription.call_id.clone(), subscription);
    }

    pub async fn unsubscribe(&self, call_id: &str) -> Option<CatalogSubscription> {
        self.subscriptions.write().await.remove(call_id)
    }

    pub async fn get(&self, call_id: &str) -> Option<CatalogSubscription> {
        self.subscriptions.read().await.get(call_id).cloned()
    }

    pub async fn get_by_device(&self, device_id: &str) -> Vec<CatalogSubscription> {
        self.subscriptions.read().await
            .values()
            .filter(|s| s.device_id == device_id)
            .cloned()
            .collect()
    }

    pub async fn get_all(&self) -> Vec<CatalogSubscription> {
        self.subscriptions.read().await.values().cloned().collect()
    }

    pub async fn cleanup_expired(&self) -> Vec<String> {
        let mut guard = self.subscriptions.write().await;
        let mut removed = Vec::new();
        
        guard.retain(|call_id, sub| {
            if sub.is_expired() {
                removed.push(call_id.clone());
                return false;
            }
            true
        });
        
        removed
    }

    pub async fn update_last_notify(&self, call_id: &str) {
        if let Some(sub) = self.subscriptions.write().await.get_mut(call_id) {
            sub.last_notify = Utc::now();
        }
    }
}

impl Default for CatalogSubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

pub fn build_catalog_notify_body(channels: &[crate::db::device::DeviceChannel], sn: u32, device_id: &str) -> String {
    let mut channel_xml = String::new();
    
    for ch in channels {
        let name = ch.name.as_deref().unwrap_or("未知通道");
        let gb_id = ch.gb_device_id.as_deref().unwrap_or("");
        let status = ch.status.as_deref().unwrap_or("OFF");
        let has_audio = ch.has_audio.unwrap_or(false);
        
        channel_xml.push_str(&format!(
            r#"<Item>
<DeviceID>{}</DeviceID>
<Name>{}</Name>
<Status>{}</Status>
<ParentID>{}</ParentID>
<Online>{}</Online>
<SubCount>{}</SubCount>
<HasAudio>{}</HasAudio>
</Item>"#,
            gb_id, name, status, device_id,
            if status == "ON" { "true" } else { "false" },
            ch.sub_count.unwrap_or(0),
            has_audio
        ));
    }
    
    let num = channels.len();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Notify>
<CmdType>Catalog</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<SumNum>{}</SumNum>
<DeviceList Num="{}">{}</DeviceList>
</Notify>"#,
        sn, device_id, num, num, channel_xml
    )
}
