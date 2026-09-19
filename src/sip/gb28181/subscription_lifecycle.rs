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

use chrono::Utc;
use dashmap::DashMap;

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
}

impl SubscriptionLifecycle {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    /// 注册一个新的 SUBSCRIBE 订阅会话
    pub fn register(
        &self,
        device_id: &str,
        sub_type: SubscriptionType,
        call_id: &str,
        expires_secs: u32,
    ) {
        let key = format!("{}_{}", device_id, sub_type.as_str());
        let renew_interval = (expires_secs / 3).max(30);
        let expires_at = Utc::now().timestamp() + expires_secs as i64;
        self.sessions.insert(
            key,
            SubscribeSession {
                device_id: device_id.to_string(),
                sub_type,
                call_id: call_id.to_string(),
                expires_at,
                renew_interval,
                active: true,
            },
        );
        tracing::info!(
            "SUBSCRIBE registered: {} {} expires={}s renew_interval={}s",
            device_id,
            sub_type.as_str(),
            expires_secs,
            renew_interval
        );
    }

    /// 接收 NOTIFY 后更新订阅会话（续期）
    pub fn renew(&self, device_id: &str, sub_type: SubscriptionType, new_expires_secs: u32) {
        let key = format!("{}_{}", device_id, sub_type.as_str());
        if let Some(mut session) = self.sessions.get_mut(&key) {
            session.expires_at = Utc::now().timestamp() + new_expires_secs as i64;
            session.renew_interval = (new_expires_secs / 3).max(30);
            session.active = true;
            tracing::debug!(
                "SUBSCRIBE renewed: {} {} expires_at={}",
                device_id,
                sub_type.as_str(),
                session.expires_at
            );
        }
    }

    /// 注销订阅
    pub fn unregister(&self, device_id: &str, sub_type: SubscriptionType) {
        let key = format!("{}_{}", device_id, sub_type.as_str());
        if let Some(mut session) = self.sessions.get_mut(&key) {
            session.active = false;
            tracing::info!(
                "SUBSCRIBE unregistered: {} {}",
                device_id,
                sub_type.as_str()
            );
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
        use crate::db::device as db_device;
        use crate::sip::gb28181::XmlParser;

        let (_sum_num, channels) = XmlParser::parse_catalog_channels(xml);
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

        tracing::info!(
            "Catalog NOTIFY processed: {} channels from {}",
            count,
            device_id
        );
        Ok(count)
    }

    /// 处理 MobilePosition NOTIFY → 落库 + Redis + WS
    pub async fn handle_position_notify(
        &self,
        xml: &str,
        redis: Option<&redis::aio::ConnectionManager>,
        ws: Option<&crate::handlers::websocket::WsState>,
    ) -> Result<(), String> {
        use crate::db::mobile_position as db_pos;
        use crate::sip::gb28181::XmlParser;

        let device_id = XmlParser::get_device_id(xml).unwrap_or_default();
        let parsed = XmlParser::parse_fields(xml);

        let latitude: Option<f64> = parsed.get("Latitude").and_then(|s| s.parse().ok());
        let longitude: Option<f64> = parsed.get("Longitude").and_then(|s| s.parse().ok());
        let speed: Option<f64> = parsed.get("Speed").and_then(|s| s.parse().ok());
        let direction: Option<i32> = parsed.get("Direction").and_then(|s| s.parse().ok());
        let gps_time = parsed
            .get("Time")
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

            // 打通「上报 → 地图可见」：地图/通道列表读的是
            // `gb_device_channel.longitude/latitude`，只写位置表的话地图上永远看不到。
            // 0,0 由该函数内部守卫（不覆盖已有坐标）。
            if let Err(e) = db_pos::sync_channel_coords(&self.pool, &device_id, lon, lat).await {
                tracing::warn!("回写通道坐标失败 device={}: {}", device_id, e);
            }

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
                w.broadcast(
                    "mobilePosition",
                    serde_json::json!({
                        "deviceId": device_id,
                        "latitude": lat,
                        "longitude": lon,
                        "speed": speed,
                    }),
                )
                .await;
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
        let alarm_type = parsed
            .get("AlarmType")
            .cloned()
            .unwrap_or_else(|| "ALARM".to_string());
        let alarm_priority = parsed
            .get("Priority")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let alarm_time = parsed
            .get("AlarmTime")
            .cloned()
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
            w.broadcast(
                "alarm",
                serde_json::json!({
                    "deviceId": device_id,
                    "alarmType": alarm_type,
                    "priority": alarm_priority,
                    "time": alarm_time,
                }),
            )
            .await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// 端到端：设备上报 MobilePosition NOTIFY 后，
    /// **位置表与通道坐标都要更新** —— 后者是地图/通道列表真正读的字段。
    ///
    /// 回归保护：此前只写 `gb_device_mobile_position`，而地图读
    /// `gb_device_channel.longitude/latitude`，导致「设备在上报、地图上看不到」。
    #[tokio::test]
    async fn position_notify_syncs_channel_coords() {
        use crate::test_support::sqlite_pool_with_schema;
        let pool = sqlite_pool_with_schema().await;
        let dev = "34020000001320128497";
        let ch = "34020000001310000001";

        sqlx::query("INSERT INTO gb_device (device_id, name, on_line) VALUES (?, ?, 1)")
            .bind(dev).bind("test-dev").execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO gb_device_channel (device_id, gb_device_id, name, status, create_time, update_time, data_type, data_device_id) \
             VALUES (?, ?, ?, 'ON', '2026-09-19 00:00:00', '2026-09-19 00:00:00', 0, 0)",
        )
        .bind(dev).bind(ch).bind("ch1").execute(&pool).await.unwrap();

        let xml = format!(
            r#"<?xml version="1.0" encoding="GB2312"?>
<Notify><CmdType>MobilePosition</CmdType><SN>1</SN><DeviceID>{dev}</DeviceID>
<Time>2026-09-19T20:00:00</Time><Longitude>116.397128</Longitude><Latitude>39.916527</Latitude>
<Speed>0</Speed><Direction>0</Direction></Notify>"#
        );
        NotifyDispatcher::new(pool.clone())
            .handle_position_notify(&xml, None, None)
            .await
            .expect("NOTIFY 处理应成功");

        // 1) 位置表有记录
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gb_device_mobile_position")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(n, 1, "位置表应有 1 条");

        // 2) 通道坐标已回写（地图读的就是这两个字段）
        let (lng, lat): (Option<f64>, Option<f64>) =
            sqlx::query_as("SELECT longitude, latitude FROM gb_device_channel WHERE gb_device_id = ?")
                .bind(ch).fetch_one(&pool).await.unwrap();
        assert_eq!(lng, Some(116.397128), "通道经度应被回写");
        assert_eq!(lat, Some(39.916527), "通道纬度应被回写");
    }

    /// 守卫：上报里没有经纬度（解析为 0,0）时**不得**覆盖已有正确坐标，
    /// 否则地图上的设备会因为 (0,0) 被判为无效点而消失。
    #[tokio::test]
    async fn position_notify_zero_coords_does_not_overwrite() {
        use crate::test_support::sqlite_pool_with_schema;
        let pool = sqlite_pool_with_schema().await;
        let dev = "34020000001320128497";
        let ch = "34020000001310000001";

        sqlx::query("INSERT INTO gb_device (device_id, name, on_line) VALUES (?, ?, 1)")
            .bind(dev).bind("test-dev").execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO gb_device_channel (device_id, gb_device_id, name, status, longitude, latitude, create_time, update_time, data_type, data_device_id) \
             VALUES (?, ?, ?, 'ON', 116.4, 39.9, '2026-09-19 00:00:00', '2026-09-19 00:00:00', 0, 0)",
        )
        .bind(dev).bind(ch).bind("ch1").execute(&pool).await.unwrap();

        let xml = format!(
            r#"<Notify><CmdType>MobilePosition</CmdType><SN>2</SN><DeviceID>{dev}</DeviceID>
<Time>2026-09-19T20:00:01</Time><Longitude>0</Longitude><Latitude>0</Latitude></Notify>"#
        );
        NotifyDispatcher::new(pool.clone())
            .handle_position_notify(&xml, None, None)
            .await
            .expect("NOTIFY 处理应成功");

        let (lng, lat): (Option<f64>, Option<f64>) =
            sqlx::query_as("SELECT longitude, latitude FROM gb_device_channel WHERE gb_device_id = ?")
                .bind(ch).fetch_one(&pool).await.unwrap();
        assert_eq!(lng, Some(116.4), "0,0 上报不得覆盖已有经度");
        assert_eq!(lat, Some(39.9), "0,0 上报不得覆盖已有纬度");
    }

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
