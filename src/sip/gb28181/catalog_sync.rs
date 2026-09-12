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

use std::sync::Arc;

use dashmap::DashMap;

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
        let _num = Self::extract_tag(xml, "Num")
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

        // GB28181 协议：<Num> 是当前包序号（1..SumNum），每收一包 +1，
        // 而非包内 item 数量；这里按包计数与总包数 SumNum 比对。
        self.received_num += 1;
        self.state = if self.received_num >= self.total_num {
            SyncState::Done
        } else {
            SyncState::Receiving
        };
        self.received_num >= self.total_num
    }

    /// 超时收尾：把会话从"等待/接收中"推进到一个**确定**的终态。
    ///
    /// 每个分页到达时都已**逐包 upsert 入库**，所以少收几页不等于数据全丢；
    /// 但会话若一直停在 `Receiving`，调用方只能永远报"同步进行中"，
    /// 前端无法判断这次同步到底结束没有。
    ///
    /// * 一页都没收到 → `Failed`（设备没响应）
    /// * 收到部分页     → `Done`，并在 `error` 里说明"声明 N 页、实收 M 页"
    ///
    /// 返回 `true` 表示本次调用确实改写了状态。
    pub fn finalize_partial(&mut self, timeout_secs: u64) -> bool {
        if !matches!(self.state, SyncState::Waiting | SyncState::Receiving) {
            return false;
        }
        if self.received_num == 0 {
            self.state = SyncState::Failed;
            self.error = Some(format!("设备在 {timeout_secs} 秒内未返回任何目录分页"));
        } else {
            self.state = SyncState::Done;
            self.error = Some(format!(
                "设备声明 {} 个分页、实收 {} 个；各分页已逐包入库，目录可能不完整",
                self.total_num, self.received_num
            ));
        }
        true
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
}

impl CatalogSyncManager {
    pub fn new(pool: Pool) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            pool,
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
                device_id,
                session.total_num,
                session.received_num
            );
            // 解析通道列表并更新 DB
            match self.flush_to_db(&session).await {
                Ok(count) => {
                    tracing::info!(
                        "Catalog channels upserted: device={} count={}",
                        device_id,
                        count
                    );
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
                device_id,
                session.received_num,
                session.total_num
            );
        }

        Ok(done)
    }

    /// 将会话缓冲中的通道数据解析并写入 DB
    async fn flush_to_db(&self, session: &CatalogSyncSession) -> Result<i32, String> {
        let device_id = &session.device_id;
        let (_, channels) = crate::sip::gb28181::XmlParser::parse_catalog_channels(&session.buffer);
        let mut count = 0;

        for ch in channels {
            let status = ch.status == "ON" || ch.status == "online";
            let parent_id = ch.parent_id.as_ref().unwrap_or(device_id);

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

    /// 超时收尾（见 [`CatalogSyncSession::finalize_partial`]）。
    pub fn finalize_partial(&self, device_id: &str, timeout_secs: u64) -> bool {
        match self.sessions.get_mut(device_id) {
            Some(mut s) => s.value_mut().finalize_partial(timeout_secs),
            None => false,
        }
    }

    /// 获取同步会话状态
    pub fn get_session(&self, device_id: &str) -> Option<CatalogSyncSession> {
        self.sessions.get(device_id).map(|r| r.value().clone())
    }

    /// 删除同步会话
    pub fn remove_session(&self, device_id: &str) {
        self.sessions.remove(device_id);
    }

    /// 取消设备所有同步会话
    pub fn cancel_all(&self) {
        self.sessions
            .retain(|_, s| s.state != SyncState::Waiting && s.state != SyncState::Receiving);
    }

    /// 获取当前所有活跃会话数
    pub fn active_count(&self) -> usize {
        self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 设备声明 3 页却只发 1 页时，会话必须能被**确定性收尾**：
    /// 收到部分 → Done（并说明差异），一页没收到 → Failed。
    #[test]
    fn finalize_partial_makes_state_deterministic() {
        let mut sess = CatalogSyncSession::new("dev-1".to_string(), 1);
        let page = r#"<?xml version="1.0"?>
<Response><CmdType>Catalog</CmdType><SN>1</SN><SumNum>3</SumNum><Num>1</Num>
<DeviceList Num="1"><Item><DeviceID>ch-1</DeviceID><Name>c1</Name></Item></DeviceList></Response>"#;
        assert!(!sess.add_packet(page), "只收到 1/3 页，不该判定收齐");
        assert_eq!(sess.state, SyncState::Receiving);

        assert!(sess.finalize_partial(8), "应改写成终态");
        assert_eq!(sess.state, SyncState::Done);
        let err = sess.error.clone().unwrap();
        assert!(err.contains("声明 3 个分页、实收 1 个"), "{err}");
        // 已经收尾过就不再改写
        assert!(!sess.finalize_partial(8));

        // 一页都没收到 → Failed
        let mut none = CatalogSyncSession::new("dev-2".to_string(), 2);
        assert_eq!(none.state, SyncState::Waiting);
        assert!(none.finalize_partial(8));
        assert_eq!(none.state, SyncState::Failed);
        assert!(none.error.unwrap().contains("未返回任何目录分页"));
    }

    /// 管理器层面的收尾：`finalize_partial` 必须真的改写 map 里的会话状态。
    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn manager_finalize_partial_rewrites_session() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        let mgr = CatalogSyncManager::new(pool);
        mgr.start_sync("dev-x", 7);
        let page = r#"<Response><CmdType>Catalog</CmdType><SN>7</SN><SumNum>9</SumNum><Num>1</Num>
<DeviceList Num="1"><Item><DeviceID>ch-1</DeviceID><Name>c1</Name></Item></DeviceList></Response>"#;
        mgr.handle_packet("dev-x", page).await;
        let sess = mgr.get_session("dev-x").unwrap();
        assert_eq!(sess.received_num, 1);
        assert_eq!(sess.state, SyncState::Receiving);

        assert!(mgr.finalize_partial("dev-x", 8), "应收尾成功");
        let sess = mgr.get_session("dev-x").unwrap();
        assert_eq!(sess.state, SyncState::Done, "必须真的落成 Done");
        assert!(sess.error.unwrap().contains("声明 9 个分页、实收 1 个"));
        // 不存在的设备不做任何事
        assert!(!mgr.finalize_partial("nope", 8));
    }

    #[test]
    fn test_catalog_sync_two_packets() {
        let _session = CatalogSyncSession::new("34020000001320000001".to_string(), 1);

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
        assert_eq!(s.received_num, 1);
        assert_eq!(s.state, SyncState::Receiving);

        let done2 = s.add_packet(page2);
        assert!(!done2);
        assert_eq!(s.received_num, 2);
        assert_eq!(s.state, SyncState::Receiving);

        let done3 = s.add_packet(page3);
        assert!(done3);
        assert_eq!(s.received_num, 3);
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
