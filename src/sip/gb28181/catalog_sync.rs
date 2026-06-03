// ! CatalogSync — GB28181 目录多包同步与订阅生命周期
//!
//! 功能：
//! 1. Catalog 查询（发送 SIP MESSAGE → 等待多包响应 → 聚合）
//! 2. Catalog 订阅（SUBSCRIBE → NOTIFY 路由 → DB 落库 → WS 广播）
//! 3. 多包聚合（SumNum/Num/SumCount）
//! 4. 订阅自动续期（后台任务）
//!
//! 与 CatalogSubscriptionManager 的关系：
//! - CatalogSubscriptionManager 管理订阅本身的状态（订阅/取消/过期）
//! - CatalogSyncManager 管理目录同步的进度（多包缓冲/完成状态/通道更新）

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::RwLock;

use crate::sip::gb28181::CatalogSubscriptionManager;
use crate::db::device as db_device;
use crate::db::Pool;

/// Catalog 同步会话状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    /// 刚发起查询，等待设备响应
    Waiting,
    /// 接收分页中（SumNum > 已收到数量）
    Receiving,
    /// 收齐所有分页，解析入库
    Done,
    /// 解析或入库失败
    Failed,
}

/// Catalog 同步会话
#[derive(Debug, Clone)]
pub struct CatalogSyncSession {
    /// 关联的设备 ID
    pub device_id: String,
    /// 流水号
    pub sn: u32,
    /// 当前会话的 SumNum（设备返回的总包数）
    pub total_num: i32,
    /// 已收到包数
    pub received_num: i32,
    /// XML 缓冲（多包聚合用）
    pub buffer: String,
    /// 当前同步状态
    pub state: SyncState,
    /// 错误信息（如有）
    pub error: Option<String>,
    /// 开始时间（秒）
    pub started_at: i64,
}

impl CatalogSyncSession {
    pub fn new(device_id: String, sn: u32) -> Self {
        Self {
            device_id,
            sn,
            total_num: 0,
            received_num: 0,
            buffer: String::new(),
            state: SyncState::Waiting,
            error: None,
            started_at: chrono::Utc::now().timestamp(),
        }
    }

    /// 追加一个 Catalog 分页包
    /// 返回 true 表示所有包已收齐
    pub fn add_packet(&mut self, xml: &str) -> bool {
        // 解析 SumNum 和 Num
        let sum_num = Self::extract_tag(xml, "SumNum")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        let num = Self::extract_tag(xml, "Num")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        if self.total_num == 0 {
            self.total_num = sum_num;
        }

        // 追加 DeviceList 内容
        if let Some(start) = xml.find("<DeviceList") {
            if let Some(end) = xml.find("</DeviceList>") {
                self.buffer.push_str(&xml[start..=end]);
            }
        }

        self.received_num += num;
        self.state = if self.received_num >= self.total_num {
            SyncState::Done
        } else {
            SyncState::Receiving
        };
        self.received_num >= self.total_num
    }

    /// 标记同步失败
    pub fn set_failed(&mut self, err: String) {
        self.state = SyncState::Failed;
        self.error = Some(err);
    }

    /// 从 XML 提取标签值
    fn extract_tag(xml: &str, tag: &str) -> Option<String> {
        let start_tag = format!("<{}>", tag);
        let end_tag = format!("</{}>", tag);
        let start_pos = xml.find(&start_tag)?;
        let end_pos = xml[start_pos..].find(&end_tag)?;
        Some(xml[start_pos + start_tag.len()..start_pos + end_pos].to_string())
    }
}

/// Catalog 同步管理器
pub struct CatalogSyncManager {
    /// 按 device_id 索引的同步会话
    sessions: Arc<DashMap<String, CatalogSyncSession>>,
    /// 数据库连接池
    pool: Pool,
    /// Catalog 订阅管理器引用
    subscription_manager: Arc<CatalogSubscriptionManager>,
}

impl CatalogSyncManager {
    pub fn new(pool: Pool, subscription_manager: Arc<CatalogSubscriptionManager>) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            pool,
            subscription_manager,
        }
    }

    /// 开始一个新的 Catalog 查询会话
    pub fn start_sync(&self, device_id: &str, sn: u32) {
        let key = device_id.to_string();
        let session = CatalogSyncSession::new(device_id.to_string(), sn);
        self.sessions.insert(key, session);
        tracing::info!("Catalog sync started: device={} sn={}", device_id, sn);
    }

    /// 处理收到的 Catalog 分页包（来自 handle_message 或 handle_notify）
    /// 返回 true 表示收齐所有包并完成入库
    pub async fn handle_packet(&self, device_id: &str, xml: &str) -> Result<bool, String> {
        let key = device_id.to_string();

        // 确保会话存在
        if !self.sessions.contains_key(&key) {
            self.start_sync(device_id, 1);
        }

        let mut session = match self.sessions.get_mut(&key) {
            Some(s) => s,
            None => return Err("Session not found".to_string()),
        };

        // 追加分页数据
        let done = session.add_packet(xml);

        if done {
            tracing::info!(
                "Catalog sync complete: device={} total={} packets={}",
                device_id, session.total_num, session.received_num
            );
            // 解析通道列表并更新 DB
            match self.flush_to_db(&session).await {
                Ok(count) => {
                    tracing::info!("Catalog channels upserted: device={} count={}", device_id, count);
                }
                Err(e) => {
                    tracing::error!("Catalog DB upsert failed: device={} err={}", device_id, e);
                    session.set_failed(e.clone());
                    return Err(e);
                }
            }
        } else {
            tracing::debug!(
                "Catalog sync progress: device={} {}/{} packets",
                device_id, session.received_num, session.total_num
            );
        }

        Ok(done)
    }

    /// 将会话缓冲中的通道数据解析并写入 DB
    async fn flush_to_db(&self, session: &CatalogSyncSession) -> Result<i32, String> {
        let device_id = &session.device_id;
        let (_total_num, channels) = crate::sip::gb28181::XmlParser::parse_catalog_channels(&session.buffer);
        let mut count = 0;

        for ch in channels {
            let status = ch.status == "ON" || ch.status == "online";
            let parent_id = ch.parent_id.as_deref().unwrap_or(device_id);

            db_device::upsert_channel_from_catalog(
                &self.pool,
                device_id,
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

        Ok(count)
    }

    /// 获取同步会话状态
    pub fn get_session(&self, device_id: &str) -> Option<CatalogSyncSession> {
        self.sessions.get(device_id).map(|r| r.clone())
    }

    /// 删除同步会话
    pub fn remove_session(&self, device_id: &str) {
        self.sessions.remove(device_id);
    }

    /// 取消设备所有同步会话
    pub fn cancel_all(&self) {
        self.sessions.retain(|_, s| {
            s.state != SyncState::Waiting && s.state != SyncState::Receiving
        });
    }

    /// 获取当前所有活跃会话数
    pub fn active_count(&self) -> usize {
        self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_sync_two_packets() {
        let session = CatalogSyncSession::new("34020000001320000001".to_string(), 1);

        let page1 = r#"<?xml version="1.0"?>
<Response>
<CmdType>Catalog</CmdType>
<SN>1</SN>
<DeviceID>34020000001320000001</DeviceID>
<SumNum>3</SumNum>
<Num>2</Num>
<DeviceList>
<Item><DeviceID>34020000001320000001001</DeviceID><Name>Cam001</Name><Status>ON</Status></Item>
<Item><DeviceID>34020000001320000001002</DeviceID><Name>Cam002</Name><Status>ON</Status></Item>
</DeviceList>
</Response>"#;

        let page2 = r#"<?xml version="1.0"?>
<Response>
<SumNum>3</SumNum>
<Num>1</Num>
<DeviceList>
<Item><DeviceID>34020000001320000001003</DeviceID><Name>Cam003</Name><Status>OFF</Status></Item>
</DeviceList>
</Response>"#;

        let page3 = r#"<?xml version="1.0"?>
<DeviceList>
<Item><DeviceID>34020000001320000001004</DeviceID><Name>Cam004</Name><Status>ON</Status></Item>
</DeviceList>
</Response>"#;

        // 两个独立 DashMap 引用模拟两包场景
        let mut s = CatalogSyncSession::new("dev1".to_string(), 1);
        assert_eq!(s.state, SyncState::Waiting);

        let done1 = s.add_packet(page1);
        assert!(!done1);
        assert_eq!(s.total_num, 3);
        assert_eq!(s.received_num, 2);
        assert_eq!(s.state, SyncState::Receiving);

        let done2 = s.add_packet(page2);
        assert!(!done2);
        assert_eq!(s.received_num, 3);
        assert_eq!(s.state, SyncState::Receiving);

        let done3 = s.add_packet(page3);
        assert!(done3);
        assert_eq!(s.state, SyncState::Done);
    }

    #[test]
    fn test_catalog_sync_single_packet() {
        let mut s = CatalogSyncSession::new("dev1".to_string(), 1);
        let single = r#"<?xml version="1.0"?>
<Response>
<SumNum>1</SumNum>
<Num>1</Num>
<DeviceList><Item><DeviceID>ch1</DeviceID><Name>Ch-1</Name></Item></DeviceList>
</Response>"#;

        let done = s.add_packet(single);
        assert!(done);
        assert_eq!(s.state, SyncState::Done);
    }

    #[test]
    fn test_sync_session_failed() {
        let mut s = CatalogSyncSession::new("dev1".to_string(), 1);
        s.set_failed("Network error".to_string());
        assert_eq!(s.state, SyncState::Failed);
        assert_eq!(s.error.as_deref(), Some("Network error"));
    }
}
