// ! JtCommandWaiter — JT1078 命令→响应关联
//!
//! 与 PendingRequestManager 的类比：
//!   SIP: PendingRequestManager  (call_id / device_id+sn)
//!   JT:  JtCommandWaiter  (phone + msg_id + serial)
//!
//! 功能：
//! 1. 命令发送时注册（phone + msg_id + serial_no）
//! 2. 收到响应时匹配并解析
//! 3. 超时清理
//! 4. 响应解析为结构化类型

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::sync::oneshot;

/// JT1078 命令类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JtCmdType {
    /// 终端注册
    Register,
    /// 注销
    Unregister,
    /// 心跳
    Heartbeat,
    /// 实时音视频传输请求
    LiveVideoStart,
    /// 实时音视频传输停止
    LiveVideoStop,
    /// 音视频实时传输控制（暂停/恢复/开关码流）
    LiveVideoControl,
    /// 文件上传
    FileUpload,
    /// 音视频文件检索
    MediaSearch,
    /// 回放请求
    PlaybackStart,
    /// 回放控制（暂停/恢复/拖动/倍速）
    PlaybackControl,
    /// 云台控制
    Ptz,
    /// 文本信息下发
    TextMessage,
    /// 拍照请求
    TakePhoto,
    /// 查询终端属性
    QueryAttributes,
    /// 查询位置信息
    QueryLocation,
    /// 位置信息上报
    LocationReport,
    /// 行驶记录仪数据上传
    DriveRecorder,
    /// 透传数据
    Transparent,
    /// 通用未知命令
    Unknown,
}

impl JtCmdType {
    pub fn from_msg_id(id: u16) -> Self {
        match id {
            0x0100 => JtCmdType::Register,
            0x0102 => JtCmdType::Unregister,
            0x0002 => JtCmdType::Heartbeat,
            0x9101 => JtCmdType::LiveVideoStart,
            0x9102 => JtCmdType::LiveVideoStop,
            0x9103 => JtCmdType::LiveVideoControl,
            0x9201 => JtCmdType::PlaybackStart,
            0x9202 => JtCmdType::PlaybackControl,
            0x8103 => JtCmdType::Ptz,
            0x8300 => JtCmdType::TextMessage,
            0x8802 => JtCmdType::TakePhoto,
            0x8108 => JtCmdType::QueryAttributes,
            0x8202 => JtCmdType::QueryLocation,
            0x0200 => JtCmdType::LocationReport,
            0x0704 => JtCmdType::DriveRecorder,
            _ => JtCmdType::Unknown,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            JtCmdType::Register => "Register",
            JtCmdType::Unregister => "Unregister",
            JtCmdType::Heartbeat => "Heartbeat",
            JtCmdType::LiveVideoStart => "LiveVideoStart",
            JtCmdType::LiveVideoStop => "LiveVideoStop",
            JtCmdType::LiveVideoControl => "LiveVideoControl",
            JtCmdType::FileUpload => "FileUpload",
            JtCmdType::MediaSearch => "MediaSearch",
            JtCmdType::PlaybackStart => "PlaybackStart",
            JtCmdType::PlaybackControl => "PlaybackControl",
            JtCmdType::Ptz => "Ptz",
            JtCmdType::TextMessage => "TextMessage",
            JtCmdType::TakePhoto => "TakePhoto",
            JtCmdType::QueryAttributes => "QueryAttributes",
            JtCmdType::QueryLocation => "QueryLocation",
            JtCmdType::LocationReport => "LocationReport",
            JtCmdType::DriveRecorder => "DriveRecorder",
            JtCmdType::Transparent => "Transparent",
            JtCmdType::Unknown => "Unknown",
        }
    }
}

/// 等待中的命令
#[derive(Debug)]
pub struct JtPendingCmd {
    /// 终端电话号码
    pub phone: String,
    /// 消息 ID
    pub msg_id: u16,
    /// 流水号
    pub serial_no: u16,
    /// 命令类型
    pub cmd_type: JtCmdType,
    /// 创建时间
    pub created_at: Instant,
    /// 超时时长
    pub timeout_secs: u64,
}

impl JtPendingCmd {
    pub fn new(phone: String, msg_id: u16, serial_no: u16, timeout_secs: u64) -> Self {
        Self {
            phone,
            msg_id,
            serial_no,
            cmd_type: JtCmdType::from_msg_id(msg_id),
            created_at: Instant::now(),
            timeout_secs,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > Duration::from_secs(self.timeout_secs)
    }
}

/// 命令等待管理器
pub struct JtCommandWaiter {
    /// 三重索引：phone + msg_id + serial → sender
    by_key: Arc<DashMap<String, oneshot::Sender<Vec<u8>>>>,
    /// 快速查找用：serial_no → key（用于不需要 phone 的场景）
    by_serial: Arc<DashMap<u16, String>>,
    /// 元数据
    metadata: Arc<DashMap<String, JtPendingCmd>>,
    /// 默认超时秒数
    default_timeout_secs: u64,
}

impl JtCommandWaiter {
    pub fn new() -> Self {
        Self {
            by_key: Arc::new(DashMap::new()),
            by_serial: Arc::new(DashMap::new()),
            metadata: Arc::new(DashMap::new()),
            default_timeout_secs: 10,
        }
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.default_timeout_secs = secs;
        self
    }

    /// 注册一个等待响应的命令
    /// 返回 (key, rx) — rx 用于等待响应数据
    pub fn register(
        &self,
        phone: &str,
        msg_id: u16,
        serial_no: u16,
        timeout_secs: Option<u64>,
    ) -> (String, oneshot::Receiver<Vec<u8>>) {
        let timeout = timeout_secs.unwrap_or(self.default_timeout_secs);
        let key = format!("{}:{:04X}:{}", phone, msg_id, serial_no);
        let (tx, rx) = oneshot::channel();
        self.by_key.insert(key.clone(), tx);
        self.by_serial.insert(serial_no, key.clone());
        self.metadata.insert(key.clone(), JtPendingCmd::new(
            phone.to_string(), msg_id, serial_no, timeout,
        ));
        (key, rx)
    }

    /// 接收端通过 phone+msg_id+serial_no 完成等待
    pub fn complete(&self, phone: &str, msg_id: u16, serial_no: u16, response: Vec<u8>) -> bool {
        let key = format!("{}:{:04X}:{}", phone, msg_id, serial_no);
        if let Some((_, tx)) = self.by_key.remove(&key) {
            let _ = tx.send(response);
            self.metadata.remove(&key);
            self.by_serial.remove(&serial_no);
            return true;
        }
        false
    }

    /// 通过 serial_no 完成等待（当 phone 不明确时）
    pub fn complete_by_serial(&self, serial_no: u16, response: Vec<u8>) -> bool {
        if let Some(key) = self.by_serial.get(&serial_no) {
            let key_str = key.clone();
            drop(key);
            return self.complete_by_key(&key_str, response);
        }
        false
    }

    fn complete_by_key(&self, key: &str, response: Vec<u8>) -> bool {
        if let Some((_, tx)) = self.by_key.remove(key) {
            let _ = tx.send(response);
            self.metadata.remove(key);
            return true;
        }
        false
    }

    /// 清理已超时的命令
    pub fn cleanup_expired(&self) -> Vec<String> {
        let mut removed = Vec::new();
        let snap: Vec<_> = self.metadata.iter().map(|r| r.key().clone()).collect();
        for key in snap {
            if let Some(meta) = self.metadata.get(&key) {
                if meta.is_expired() {
                    self.by_key.remove(&key);
                    self.metadata.remove(&key);
                    removed.push(key.clone());
                }
            }
        }
        removed
    }

    /// 当前等待中的命令数
    pub fn pending_count(&self) -> usize {
        self.by_key.len()
    }

    /// 终端是否有等待中的命令
    pub fn has_pending_for_phone(&self, phone: &str) -> bool {
        self.metadata.iter().any(|r| r.phone == phone)
    }
}

impl Default for JtCommandWaiter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 响应解析
// ---------------------------------------------------------------------------

/// JT1078 响应解析结果
#[derive(Debug)]
pub enum JtResponse {
    /// 成功（带消息体）
    Ok(Vec<u8>),
    /// 通用应答（0=成功 / 1=失败）
    GeneralAck { success: bool },
    /// 位置数据
    Location { lat: f64, lon: f64, speed: f64, time: String },
    /// 音视频类型
    MediaType { media_type: u8 },
    /// 未知类型
    Unknown(Vec<u8>),
}

impl JtCommandWaiter {
    /// 解析响应体
    pub fn parse_response(&self, cmd_type: JtCmdType, body: &[u8]) -> JtResponse {
        match cmd_type {
            JtCmdType::Heartbeat => JtResponse::GeneralAck { success: true },
            JtCmdType::QueryLocation => self.parse_location(body),
            JtCmdType::LocationReport => self.parse_location(body),
            JtCmdType::LiveVideoStart | JtCmdType::LiveVideoStop |
            JtCmdType::LiveVideoControl | JtCmdType::PlaybackStart |
            JtCmdType::PlaybackControl => {
                // 通用应答：第 3 个字节（0=成功）
                if body.len() >= 3 {
                    let result = body[2];
                    JtResponse::GeneralAck { success: result == 0x00 }
                } else {
                    JtResponse::Unknown(body.to_vec())
                }
            }
            _ => JtResponse::Unknown(body.to_vec()),
        }
    }

    fn parse_location(&self, body: &[u8]) -> JtResponse {
        // JT808 位置信息体（简化版）：23 字节固定 + 可变报警/扩展
        // 纬度：4 字节 int32（1e-6 度），经度：4 字节 int32，速度：2 字节 u16
        if body.len() < 28 {
            return JtResponse::Unknown(body.to_vec());
        }
        // 位置基本信息 word0-word6 (bytes 0-13)
        let lat_raw = i32::from_be_bytes([body[0], body[1], body[2], body[3]]);
        let lon_raw = i32::from_be_bytes([body[4], body[5], body[6], body[7]]);
        let lat = lat_raw as f64 / 1_000_000.0;
        let lon = lon_raw as f64 / 1_000_000.0;
        let speed_raw = u16::from_be_bytes([body[14], body[15]]);
        let speed = speed_raw as f64;
        // 时间：bytes 20-26，BCD 格式 YYMMDDHHmmss
        let time = format!(
            "20{}{}/{}/{}/{} {}:{}:{}",
            hex_digit(body[20]), hex_digit(body[21]),
            hex_digit(body[22]), hex_digit(body[23]),
            hex_digit(body[24]),
            hex_digit(body[25]), hex_digit(body[26]), hex_digit(body[27])
        );

        JtResponse::Location { lat, lon, speed, time }
    }
}

fn hex_digit(b: u8) -> String {
    format!("{:02X}", b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_complete() {
        let waiter = JtCommandWaiter::new();
        let (key, rx) = waiter.register("13812340001", 0x9101, 1, None);

        assert_eq!(waiter.pending_count(), 1);
        assert!(waiter.has_pending_for_phone("13812340001"));

        let body = vec![0u8; 3];
        let result = waiter.complete("13812340001", 0x9101, 1, body.clone());
        assert!(result);
        assert_eq!(waiter.pending_count(), 0);

        let received = rx.await.unwrap();
        assert_eq!(received, body);
    }

    #[tokio::test]
    async fn test_timeout_cleanup() {
        let mut waiter = JtCommandWaiter::new();
        waiter.default_timeout_secs = 0; // 0s = 立即过期
        waiter.register("13812340001", 0x9101, 1, None);
        assert_eq!(waiter.pending_count(), 1);

        std::thread::sleep(Duration::from_millis(10));
        let removed = waiter.cleanup_expired();
        assert_eq!(removed.len(), 1);
        assert_eq!(waiter.pending_count(), 0);
    }

    #[test]
    fn test_parse_location() {
        let waiter = JtCommandWaiter::new();
        // 模拟：lat=30.0, lon=120.0, speed=60
        let mut body = vec![0u8; 28];
        let lat = (30.0 * 1_000_000.0) as i32;
        let lon = (120.0 * 1_000_000.0) as i32;
        body[0..4].copy_from_slice(&lat.to_be_bytes());
        body[4..8].copy_from_slice(&lon.to_be_bytes());
        body[14..16].copy_from_slice(&60u16.to_be_bytes());
        // BCD time: 26/05/31/12/00/00
        body[20] = 0x26; body[21] = 0x05; body[22] = 0x31;
        body[23] = 0x12; body[24] = 0x00; body[25] = 0x00; body[26] = 0x00; body[27] = 0x00;

        let result = waiter.parse_response(JtCmdType::LocationReport, &body);
        match result {
            JtResponse::Location { lat, lon, speed, .. } => {
                assert!((lat - 30.0).abs() < 0.001);
                assert!((lon - 120.0).abs() < 0.001);
                assert_eq!(speed, 60.0);
            }
            _ => panic!("Expected Location response"),
        }
    }

    #[test]
    fn test_cmd_type_from_msg_id() {
        assert_eq!(JtCmdType::from_msg_id(0x0100), JtCmdType::Register);
        assert_eq!(JtCmdType::from_msg_id(0x9101), JtCmdType::LiveVideoStart);
        assert_eq!(JtCmdType::from_msg_id(0x9201), JtCmdType::PlaybackStart);
        assert_eq!(JtCmdType::from_msg_id(0x8103), JtCmdType::Ptz);
        assert_eq!(JtCmdType::from_msg_id(0x8202), JtCmdType::QueryLocation);
    }
}
