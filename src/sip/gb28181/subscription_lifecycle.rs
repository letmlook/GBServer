// ! SubscriptionLifecycle — 订阅生命周期（订阅发送 + NOTIFY 解析 + Redis + WS）
//!
//! 对应 GB28181 SUBSCRIBE/NOTIFY 机制：
//! 1. 发起订阅（SUBSCRIBE）
//! 2. 接收通知（NOTIFY → 解析 → DB + Redis + WS）
//! 3. 续期（自动发送 SUBSCRIBE 刷新）
//! 4. 取消订阅
//!
//! 支持订阅类型：Catalog / MobilePosition / Alarm

use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use chrono::Utc;

use crate::sip::gb28181::SubscriptionType;
use crate::db::Pool;

/// 已发送的 SUBSCRIBE 会话（管理续期）
#[derive(Debug, Clone)]
pub struct SubscribeSession {
    pub device_id: String,
    pub sub_type: SubscriptionType,
    pub call_id: String,
    /// 到期时间戳（秒）
    pub expires_at: i64,
    /// 续期间隔（秒），默认 1/3 expires
    pub renew_interval: u32,
    /// 活跃标记
    pub active: bool,
}

impl SubscribeSession {
    pub fn needs_renew(&self) -> bool {
        let now = Utc::now().timestamp();
        let remaining = self.expires_at - now;
        remaining <= 30 && remaining > 0 && self.active
    }

    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        self.expires_at <= now || !self.active
    }
}

/// SUBSCRIBE 生命周期管理器
pub struct SubscriptionLifecycle {
    /// 按 device_id + sub_type 索引的订阅会话
    sessions: Arc<DashMap<String, SubscribeSession>>,
    /// 续期间隔（秒）
    renew_interval_secs: u32,
}

impl SubscriptionLifecycle {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            renew_interval_secs: 30,
        }
    }

    /// 注册一个新的 SUBSCRIBE 订阅会话
    pub fn register(&self, device_id: &str, sub_type: SubscriptionType, call_id: &str, expires_secs: u32) {
        let key = format!("{}_{}", device_id, sub_type.as_str());
        let renew_interval = (expires_secs / 3).max(30);
        let expires_at = Utc::now().timestamp() + expires_secs as i64;
        self.sessions.insert(key, SubscribeSession {
            device_id: device_id.to_string(),
            sub_type,
            call_id: call_id.to_string(),
            expires_at,
            renew_interval,
            active: true,
        });
        tracing::info!("SUBSCRIBE registered: {} {} expires={}s renew_interval={}s",
            device_id, sub_type.as_str(), expires_secs, renew_interval);
    }

    /// 接收 NOTIFY 后更新订阅会话（续期）
    pub fn renew(&self, device_id: &str, sub_type: SubscriptionType, new_expires_secs: u32) {
        let key = format!("{}_{}", device_id, sub_type.as_str());
        if let Some(mut session) = self.sessions.get_mut(&key) {
            session.expires_at = Utc::now().timestamp() + new_expires_secs as i64;
            session.renew_interval = (new_expires_secs / 3).max(30);
            session.active = true;
            tracing::debug!("SUBSCRIBE renewed: {} {} expires_at={}",
                device_id, sub_type.as_str(), session.expires_at);
        }
    }

    /// 注销订阅
    pub fn unregister(&self, device_id: &str, sub_type: SubscriptionType) {
        let key = format!("{}_{}", device_id, sub_type.as_str());
        if let Some(mut session) = self.sessions.get_mut(&key) {
            session.active = false;
            tracing::info!("SUBSCRIBE unregistered: {} {}", device_id, sub_type.as_str());
        }
    }

    /// 获取需要续期的订阅列表
    pub fn get_needing_renew(&self) -> Vec<SubscribeSession> {
        self.sessions
            .iter()
            .filter(|r| r.needs_renew())
            .map(|r| r.clone())
            .collect()
    }

    /// 获取设备所有活跃订阅
    pub fn get_for_device(&self, device_id: &str) -> Vec<SubscribeSession> {
        self.sessions
            .iter()
            .filter(|r| r.device_id == device_id && r.active)
            .map(|r| r.clone())
            .collect()
    }

    /// 获取所有活跃订阅数
    pub fn active_count(&self) -> usize {
        self.sessions.iter().filter(|r| r.active).count()
    }

    /// 清理已过期的订阅
    pub fn cleanup_expired(&self) -> Vec<String> {
        let mut removed = Vec::new();
        self.sessions.retain(|key, session| {
            if session.is_expired() {
                removed.push(key.clone());
                return false;
            }
            true
        });
        removed
    }
}

impl Default for SubscriptionLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// NOTIFY 消息处理器
// ---------------------------------------------------------------------------

/// 解析 NOTIFY 消息并分发到正确的处理函数
pub struct NotifyDispatcher {
    pool: Pool,
}

impl NotifyDispatcher {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// 解析 NOTIFY 消息，提取命令类型和数据
    /// 返回 (cmd_type, device_id, xml_body)
    pub fn parse_notify(&self, xml: &str) -> Option<(String, String, String)> {
        use crate::sip::gb28181::XmlParser;
        let cmd_type = XmlParser::get_cmd_type(xml)?;
        let device_id = XmlParser::get_device_id(xml)?;
        Some((cmd_type, device_id, xml.to_string()))
    }

    /// 处理 Catalog NOTIFY → 更新 DB + WS 广播
    pub async fn handle_catalog_notify(&self, xml: &str) -> Result<i32, String> {
        use crate::sip::gb28181::XmlParser;
        use crate::db::device as db_device;

        let (sum_num, channels) = XmlParser::parse_catalog_channels(xml);
        let device_id = XmlParser::get_device_id(xml).unwrap_or_default();

        let mut count = 0;
        for ch in &channels {
            let status = ch.status == "ON" || ch.status == "online";
            let parent_id = ch.parent_id.as_deref().unwrap_or(&device_id);
            db_device::upsert_channel_from_catalog(
                &self.pool,
                &device_id,
                &ch.device_id,
                &ch.name,
                ch.manufacturer.as_deref(),
                ch.model.as_deref(),
                ch.owner.as_deref(),
                ch.civil_code.as_deref(),
                ch.address.as_deref(),
                Some(parent_id),
                status,
                ch.longitude,
                ch.latitude,
                ch.ptz_type,
                ch.has_audio,
                ch.sub_count,
            )
            .await
            .map_err(|e| e.to_string())?;
            count += 1;
        }

        tracing::info!("Catalog NOTIFY processed: {} channels from {}", count, device_id);
        Ok(count)
    }

    /// 处理 MobilePosition NOTIFY → 落库 + Redis + WS
    pub async fn handle_position_notify(&self, xml: &str, redis: Option<&redis::aio::ConnectionManager>, ws: Option<&crate::handlers::websocket::WsState>) -> Result<(), String> {
        use crate::db::mobile_position as db_pos;
        use crate::sip::gb28181::XmlParser;

        let device_id = XmlParser::get_device_id(xml).unwrap_or_default();
        let parsed = XmlParser::parse_fields(xml);

        let latitude: Option<f64> = parsed.get("Latitude")
            .and_then(|s| s.parse().ok());
        let longitude: Option<f64> = parsed.get("Longitude")
            .and_then(|s| s.parse().ok());
        let speed: Option<f64> = parsed.get("Speed")
            .and_then(|s| s.parse().ok());
        let direction: Option<i32> = parsed.get("Direction")
            .and_then(|s| s.parse().ok());
        let gps_time = parsed.get("Time")
            .cloned()
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());

        if let (Some(lat), Some(lon)) = (latitude, longitude) {
            let record = db_pos::MobilePositionInsert {
                device_id: device_id.clone(),
                channel_id: device_id.clone(),
                longitude: Some(lon),
                latitude: Some(lat),
                speed,
                direction: direction.map(|d| d as f64),
                time: Some(gps_time.clone()),
                device_name: None,
                altitude: None,
                report_source: None,
                create_time: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            };
            db_pos::insert(&self.pool, &record)
                .await
                .map_err(|e| e.to_string())?;

            // Redis 发布
            if let Some(r) = redis {
                let channel = format!("position:{}", device_id);
                let msg = serde_json::json!({
                    "deviceId": device_id,
                    "latitude": lat,
                    "longitude": lon,
                    "speed": speed,
                    "time": gps_time,
                });
                use redis::AsyncCommands;
            let mut conn = r.clone();
            let _: Result<(), _> = conn.publish::<_, _, ()>(&channel, &msg.to_string()).await;
            }

            // WS 广播
            if let Some(w) = ws {
                w.broadcast("mobilePosition", serde_json::json!({
                    "deviceId": device_id,
                    "latitude": lat,
                    "longitude": lon,
                    "speed": speed,
                })).await;
            }
        }

        Ok(())
    }

    /// 处理 Alarm NOTIFY → 落库 + Redis + WS
    #[allow(unused_variables)]
    pub async fn handle_alarm_notify(&self, xml: &str, redis: Option<&redis::aio::ConnectionManager>, ws: Option<&crate::handlers::websocket::WsState>) -> Result<(), String> {
        use crate::db::alarm as db_alarm;
        use crate::sip::gb28181::XmlParser;

        let device_id = XmlParser::get_device_id(xml).unwrap_or_default();
        let parsed = XmlParser::parse_fields(xml);
        let alarm_type = parsed.get("AlarmType").cloned().unwrap_or_else(|| "ALARM".to_string());
        let alarm_priority = parsed.get("Priority").and_then(|s| s.parse().ok()).unwrap_or(0);
        let alarm_time = parsed.get("AlarmTime").cloned()
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());

        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let record = db_alarm::AlarmInsert {
            device_id: device_id.clone(),
            channel_id: device_id.clone(),
            alarm_type: Some(alarm_type.clone()),
            alarm_priority: Some(alarm_priority.to_string()),
            alarm_time: Some(alarm_time.clone()),
            alarm_method: Some("GB28181".to_string()),
            alarm_description: None,
            longitude: None,
            latitude: None,
            create_time: now,
        };

        db_alarm::insert_alarm(&self.pool, &record)
            .await
            .map_err(|e| e.to_string())?;

        // Phase 2.3: Redis 广播到 alarm:{device_id} 频道
        if let Some(r) = redis {
            use redis::AsyncCommands;
            let channel = format!("alarm:{}", device_id);
            let msg = serde_json::json!({
                "deviceId": device_id,
                "alarmType": alarm_type,
                "priority": alarm_priority,
                "time": alarm_time,
            });
            let mut conn = r.clone();
            let _: Result<(), _> = conn.publish::<_, _, ()>(&channel, &msg.to_string()).await;
        }

        // WS 广播
        if let Some(w) = ws {
            w.broadcast("alarm", serde_json::json!({
                "deviceId": device_id,
                "alarmType": alarm_type,
                "priority": alarm_priority,
                "time": alarm_time,
            })).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_register_and_renew() {
        let mgr = SubscriptionLifecycle::new();
        mgr.register("dev1", SubscriptionType::Catalog, "call-abc", 300);
        assert_eq!(mgr.active_count(), 1);

        // 续期
        mgr.renew("dev1", SubscriptionType::Catalog, 300);
        assert_eq!(mgr.active_count(), 1);

        // 注销
        mgr.unregister("dev1", SubscriptionType::Catalog);
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_needs_renew() {
        let mgr = SubscriptionLifecycle::new();
        mgr.register("dev1", SubscriptionType::Catalog, "call-abc", 300);
        // 新注册的不会需要续期（expires > 30s）
        let needing = mgr.get_needing_renew();
        assert!(needing.is_empty());
    }

    #[test]
    fn test_cleanup_expired() {
        let mgr = SubscriptionLifecycle::new();
        mgr.register("dev1", SubscriptionType::Catalog, "call-abc", 0); // 0s = 立即过期
        assert_eq!(mgr.active_count(), 1);
        std::thread::sleep(Duration::from_millis(10));
        let removed = mgr.cleanup_expired();
        assert_eq!(removed.len(), 1);
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_subscribe_session_needs_renew() {
        let session = SubscribeSession {
            device_id: "dev1".to_string(),
            sub_type: SubscriptionType::MobilePosition,
            call_id: "call1".to_string(),
            expires_at: chrono::Utc::now().timestamp() + 10, // 10s 后过期
            renew_interval: 30,
            active: true,
        };
        assert!(session.needs_renew());
        assert!(!session.is_expired());
    }
}
