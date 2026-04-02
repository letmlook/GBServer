use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct CascadePlatform {
    pub id: i64,
    pub platform_id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub device_id: String,
    pub password: String,
    pub online: bool,
    pub register_time: Option<DateTime<Utc>>,
    pub keepalive_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CascadeChannel {
    pub id: i64,
    pub platform_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub custom_channel_id: Option<String>,
    pub stream_mode: String,
    pub push_status: PushStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PushStatus {
    Idle,
    Pushing,
    Error,
}

pub struct CascadeManager {
    platforms: Arc<RwLock<HashMap<String, CascadePlatform>>>,
    channels: Arc<RwLock<HashMap<String, CascadeChannel>>>,
}

impl CascadeManager {
    pub fn new() -> Self {
        Self {
            platforms: Arc::new(RwLock::new(HashMap::new())),
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_platform(&self, platform: CascadePlatform) {
        self.platforms.write().await.insert(platform.platform_id.clone(), platform);
    }

    pub async fn remove_platform(&self, platform_id: &str) {
        self.platforms.write().await.remove(platform_id);
    }

    pub async fn get_platform(&self, platform_id: &str) -> Option<CascadePlatform> {
        self.platforms.read().await.get(platform_id).cloned()
    }

    pub async fn list_platforms(&self) -> Vec<CascadePlatform> {
        self.platforms.read().await.values().cloned().collect()
    }

    pub async fn set_online(&self, platform_id: &str, online: bool) {
        let mut guard = self.platforms.write().await;
        if let Some(p) = guard.get_mut(platform_id) {
            p.online = online;
            if online {
                p.register_time = Some(Utc::now());
                p.keepalive_time = Some(Utc::now());
            }
        }
    }

    pub async fn update_keepalive(&self, platform_id: &str) {
        let mut guard = self.platforms.write().await;
        if let Some(p) = guard.get_mut(platform_id) {
            p.keepalive_time = Some(Utc::now());
            p.online = true;
        }
    }

    pub async fn add_channel(&self, channel: CascadeChannel) {
        let key = format!("{}_{}", channel.platform_id, channel.channel_id);
        self.channels.write().await.insert(key, channel);
    }

    pub async fn remove_channel(&self, platform_id: &str, channel_id: &str) {
        let key = format!("{}_{}", platform_id, channel_id);
        self.channels.write().await.remove(&key);
    }

    pub async fn list_channels(&self, platform_id: &str) -> Vec<CascadeChannel> {
        self.channels.read().await
            .values()
            .filter(|c| c.platform_id == platform_id)
            .cloned()
            .collect()
    }

    pub async fn update_push_status(&self, platform_id: &str, channel_id: &str, status: PushStatus) {
        let key = format!("{}_{}", platform_id, channel_id);
        let mut guard = self.channels.write().await;
        if let Some(c) = guard.get_mut(&key) {
            c.push_status = status;
        }
    }
}

impl Default for CascadeManager {
    fn default() -> Self {
        Self::new()
    }
}

pub fn build_register_request(
    local_id: &str,
    remote_host: &str,
    remote_port: u16,
    username: &str,
    password: &str,
) -> String {
    format!(
        "REGISTER sip:{}:{} SIP/2.0\r\n\
         Via: SIP/2.0/UDP 127.0.0.1:5060;rport;branch=z9hG4bK{}\r\n\
         From: <sip:{}@127.0.0.1>;tag=local\r\n\
         To: <sip:{}@{}:{}>\r\n\
         Call-ID: cascade_{}\r\n\
         CSeq: 1 REGISTER\r\n\
         Max-Forwards: 70\r\n\
         Expires: 3600\r\n\
         User-Agent: GBServer/1.0\r\n\
         Content-Length: 0\r\n\r\n",
        remote_host,
        remote_port,
        generate_branch(),
        local_id,
        username,
        remote_host,
        remote_port,
        generate_call_id()
    )
}

pub fn build_invite_request(
    local_id: &str,
    remote_id: &str,
    remote_host: &str,
    remote_port: u16,
    channel_id: &str,
    sdp: &str,
) -> String {
    format!(
        "INVITE sip:{}@{}:{} SIP/2.0\r\n\
         Via: SIP/2.0/UDP 127.0.0.1:5060;rport;branch=z9hG4bK{}\r\n\
         From: <sip:{}@127.0.0.1>;tag=local\r\n\
         To: <sip:{}@{}:{}>\r\n\
         Call-ID: invite_{}\r\n\
         CSeq: 1 INVITE\r\n\
         Max-Forwards: 70\r\n\
         Subject: {}:0\r\n\
         User-Agent: GBServer/1.0\r\n\
         Content-Type: application/sdp\r\n\
         Content-Length: {}\r\n\r\n\
         {}",
        remote_id,
        remote_host,
        remote_port,
        generate_branch(),
        local_id,
        remote_id,
        remote_host,
        remote_port,
        generate_call_id(),
        channel_id,
        sdp.len(),
        sdp
    )
}

fn generate_branch() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:x}", rng.gen::<u32>())
}

fn generate_call_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:x}{:x}", rng.gen::<u64>(), rng.gen::<u64>())
}

pub fn build_catalog_notify(platform_id: &str, channels: &[CascadeChannel]) -> String {
    let mut items = String::new();
    for ch in channels {
        items.push_str(&format!(
            r#"<Item>
            <DeviceID>{}</DeviceID>
            <Name>Channel {}</Name>
            <Status>ON</Status>
            </Item>"#,
            ch.channel_id,
            &ch.channel_id[ch.channel_id.len().saturating_sub(10)..]
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Notify>
    <CmdType>Catalog</CmdType>
    <SN>1</SN>
    <DeviceID>{}</DeviceID>
    <SumNum>{}</SumNum>
    <DeviceList Num="{}">
        {}
    </DeviceList>
</Notify>"#,
        platform_id,
        channels.len(),
        channels.len(),
        items
    )
}
