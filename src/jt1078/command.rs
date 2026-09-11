// JT808/JT1078 command encoding
// Implements the JT808 message wrapper and common JT1078 command bodies
// JT808 format: 0x7E [msg_id:u16] [body_attrs:u16] [phone:6 BCD bytes] [seq:u16] [body...] [checksum] 0x7E

/// Build a complete JT808 frame from message ID, phone number, sequence, and body bytes.
/// Returns the full byte vector ready to send over TCP/UDP.
pub fn build_jt808_frame(msg_id: u16, phone: &str, seq: u16, body: &[u8]) -> Vec<u8> {
    let phone_bcd = phone_to_bcd(phone);
    let body_len = body.len() as u16;
    // Body attributes: bit 0-9 = body length, bit 10-12 = encryption (0=none), bit 13 = sub-package flag
    let body_attrs: u16 = body_len & 0x03FF;

    let mut frame = Vec::with_capacity(body.len() + 20);
    frame.push(0x7E);
    frame.extend_from_slice(&msg_id.to_be_bytes());
    frame.extend_from_slice(&body_attrs.to_be_bytes());
    frame.extend_from_slice(&phone_bcd);
    frame.extend_from_slice(&seq.to_be_bytes());
    // Escape: replace 0x7E with 0x7D 0x02, 0x7D with 0x7D 0x01
    for &b in body {
        match b {
            0x7E => { frame.push(0x7D); frame.push(0x02); }
            0x7D => { frame.push(0x7D); frame.push(0x01); }
            _ => frame.push(b),
        }
    }
    // Checksum: XOR of all bytes between 0x7E markers (excluding them)
    let checksum = frame[1..].iter().fold(0u8, |acc, &b| acc ^ b);
    // 0x7E in checksum is also escaped
    match checksum {
        0x7E => { frame.push(0x7D); frame.push(0x02); }
        0x7D => { frame.push(0x7D); frame.push(0x01); }
        _ => frame.push(checksum),
    }
    frame.push(0x7E);
    frame
}

/// Convert phone number string (e.g. "13812345678") to BCD bytes (6 bytes, padded with 0)
fn phone_to_bcd(phone: &str) -> [u8; 6] {
    let mut bcd = [0u8; 6];
    let digits: Vec<u8> = phone.chars()
        .filter(|c| c.is_ascii_digit())
        .take(12)
        .map(|c| c as u8 - b'0')
        .collect();
    for i in 0..6 {
        let hi = digits.get(i * 2).copied().unwrap_or(0);
        let lo = digits.get(i * 2 + 1).copied().unwrap_or(0);
        bcd[i] = (hi << 4) | lo;
    }
    bcd
}

/// Parse BCD bytes back to phone number string
pub fn bcd_to_phone(bcd: &[u8; 6]) -> String {
    let mut s = String::with_capacity(12);
    for &b in bcd.iter() {
        s.push(((b >> 4) + b'0') as char);
        s.push(((b & 0x0F) + b'0') as char);
    }
    s.trim_end_matches('0').to_string()
}

// ── JT1078 command builders ──

/// 0x9101: Live video request
/// channel_id: channel number (1-based)
/// stream_type: 0=main, 1=sub
/// close: true to close, false to open
pub fn build_live_video_request(channel_id: u8, stream_type: u8, close: bool) -> Vec<u8> {
    let cmd = if close { 1u8 } else { 0u8 };
    vec![channel_id, cmd, stream_type]
}

/// 0x9102: Live video control
/// channel_id: channel number
/// control: 0=close audio, 1=close video, 2=close all, 3=pause, 4=resume, 5=close bidirectional talk
pub fn build_live_video_control(channel_id: u8, control: u8, close: bool) -> Vec<u8> {
    vec![channel_id, control, if close { 0u8 } else { 1u8 }, 0u8]
}

/// 0x9201: Playback request
pub fn build_playback_request(channel_id: u8, stream_type: u8, storage_type: u8,
    playback_mode: u8, speed: u8, start_time: &[u8; 6], end_time: &[u8; 6]) -> Vec<u8> {
    let mut body = Vec::with_capacity(16);
    body.push(channel_id);
    body.push(stream_type);
    body.push(storage_type);
    body.push(playback_mode);
    body.push(speed);
    body.extend_from_slice(start_time);
    body.extend_from_slice(end_time);
    body
}

/// 把时间字符串/时间戳编码为 JT/T 808 的 **BCD[6]**（YY MM DD HH mm ss）。
///
/// **真 BCD**：2026-01-02 03:04:05 → `26 01 02 03 04 05`（十六进制表示）。
/// 解析失败会**静默回退到当前时间**；需要严格校验的调用方请用
/// [`try_encode_time_bcd`]。
pub fn encode_time_bcd(time_str: &str) -> [u8; 6] {
    try_encode_time_bcd(time_str).unwrap_or_else(|| bcd_from_utc(chrono::Utc::now()))
}

/// 把 UTC 时间编码成 JT/T 808 的 **BCD[6]**（YY MM DD HH mm ss，每字节两位十进制）。
///
/// **真 BCD**：2026 → `0x26`，9 月 → `0x09`。
/// 此前返回的是**原始数值**（"%y" = 26 → 字节 `0x1A`），与平台自己的
/// `parse_bcd_datetime`（按半字节解码）以及国标都不一致 —— 下发给终端的
/// 检索/回放时间段全是错的；"平台内自测"因为收发两边都错反而看不出来，
/// 本轮写 0x0802 往返测试时才暴露。
fn bcd_from_utc(dt: chrono::DateTime<chrono::Utc>) -> [u8; 6] {
    fn two_digits(value: u8) -> u8 {
        ((value / 10) << 4) | (value % 10)
    }
    let num = |fmt: &str, fallback: u8| -> u8 {
        dt.format(fmt)
            .to_string()
            .parse::<u8>()
            .map(two_digits)
            .unwrap_or(fallback)
    };
    [
        num("%y", 0x00),
        num("%m", 0x01),
        num("%d", 0x01),
        num("%H", 0x00),
        num("%M", 0x00),
        num("%S", 0x00),
    ]
}

/// **严格**解析时间字符串为 BCD[6]；无法解析返回 `None`。
///
/// 与 [`encode_time_bcd`] 的区别：后者解析失败会**静默回退到当前时间**，
/// 这在「录像下载」这类必须先确认时间段的场景下会把**错误的时间范围**下发到终端。
/// 需要先校验再下发的调用方应使用本函数。
///
/// 支持：Unix 秒 / 毫秒时间戳、`%Y-%m-%dT%H:%M:%S`、`%Y-%m-%d %H:%M:%S`、`%Y-%m-%d`。
pub fn try_encode_time_bcd(time_str: &str) -> Option<[u8; 6]> {
    use chrono::{NaiveDate, NaiveDateTime, TimeZone};

    let s = time_str.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(ts) = s.parse::<i64>() {
        let ts = if ts > 1_000_000_000_000 { ts / 1000 } else { ts };
        return chrono::DateTime::from_timestamp(ts, 0).map(bcd_from_utc);
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(bcd_from_utc(chrono::Utc.from_utc_datetime(&dt)));
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let dt = d.and_hms_opt(0, 0, 0)?;
        return Some(bcd_from_utc(chrono::Utc.from_utc_datetime(&dt)));
    }
    None
}

/// 0x9202: Playback control
pub fn build_playback_control(channel_id: u8, control: u8, speed: u8, seek_time: &[u8; 6]) -> Vec<u8> {    let mut body = Vec::with_capacity(9);
    body.push(channel_id);
    body.push(control);
    body.push(speed);
    body.extend_from_slice(seek_time);
    body
}

/// 0x9301: PTZ control
/// PTZ command bytes as per GB28181 PTZ spec
pub fn build_ptz_control(channel_id: u8, cmd_byte1: u8, cmd_byte2: u8,
    speed_h: u8, speed_v: u8, speed_z: u8) -> Vec<u8> {
    vec![channel_id, cmd_byte1, cmd_byte2, speed_h, speed_v, speed_z]
}

/// Build a standard PTZ command from direction string
pub fn ptz_direction_bytes(direction: &str, speed: u8) -> (u8, u8, u8, u8) {
    match direction.to_ascii_uppercase().as_str() {
        "UP" => (0x05, 0x01, speed, 0x00),
        "DOWN" => (0x05, 0x01, 0x00, speed),
        "LEFT" => (0x05, 0x02, speed, 0x00),
        "RIGHT" => (0x05, 0x02, 0x00, speed),
        "ZOOM_IN" | "ZOOMIN" => (0x05, 0x04, speed, 0x00),
        "ZOOM_OUT" | "ZOOMOUT" => (0x05, 0x04, 0x00, speed),
        "FOCUS_IN" | "FOCUSIN" => (0x05, 0x08, speed, 0x00),
        "FOCUS_OUT" | "FOCUSOUT" => (0x05, 0x08, 0x00, speed),
        "IRIS_IN" | "IRISIN" => (0x05, 0x10, speed, 0x00),
        "IRIS_OUT" | "IRISOUT" => (0x05, 0x10, 0x00, speed),
        "STOP" => (0x05, 0x00, 0x00, 0x00),
        _ => (0x05, 0x00, speed, 0x00),
    }
}

/// 0x8103: Set terminal parameters
pub fn build_set_params(params: &[(u32, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    body.push(params.len() as u8);
    for (id, val) in params {
        body.extend_from_slice(&id.to_be_bytes());
        body.push(val.len() as u8);
        body.extend_from_slice(val);
    }
    body
}

/// 0x8104: Query terminal parameters
pub fn build_query_params(param_ids: &[u32]) -> Vec<u8> {
    let mut body = Vec::new();
    body.push(param_ids.len() as u8);
    for id in param_ids {
        body.extend_from_slice(&id.to_be_bytes());
    }
    body
}

/// 0x8201: Query location
pub fn build_query_location() -> Vec<u8> {
    vec![]
}

/// 0x8300: Text message
pub fn build_text_message(text: &str, emergency: bool) -> Vec<u8> {
    let text_bytes = text.as_bytes();
    let mut body = Vec::with_capacity(6 + text_bytes.len());
    let flag = if emergency { 1u8 } else { 0u8 }; // bit0=emergency, bit3=terminal TTS, bit4=screen display
    body.push(flag);
    // Phone number for callback (empty)
    body.extend_from_slice(&[0u8; 5]);
    body.extend_from_slice(text_bytes);
    body
}

/// 0x8400: Phone callback
pub fn build_phone_callback(sign: u8, phone: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(7);
    body.push(sign); // 0=hangup, 1=callback, 2=monitor, 3=listen, 4=broadcast
    let phone_bcd = phone_to_bcd(phone);
    body.extend_from_slice(&phone_bcd);
    body
}

/// 0x8500: Vehicle control (door, etc.)
pub fn build_vehicle_control(control_type: u8, value: bool) -> Vec<u8> {
    vec![control_type, if value { 1u8 } else { 0u8 }]
}

/// 0x8604: Set terminal parameters - wiper control
pub fn build_wiper_control(on: bool) -> Vec<u8> {
    let param_id: u32 = 0x0015; // wiper parameter ID
    let val = if on { 1u8 } else { 0u8 };
    build_set_params(&[(param_id, &[val])])
}

/// 0x8606: Set terminal parameters - fill light control
pub fn build_fill_light_control(on: bool) -> Vec<u8> {
    let param_id: u32 = 0x0016;
    let val = if on { 1u8 } else { 0u8 };
    build_set_params(&[(param_id, &[val])])
}

/// 0x8105: Terminal control (reset, factory reset)
pub fn build_terminal_control(cmd: u8) -> Vec<u8> {
    vec![cmd] // 1=upgrade, 2=restart, 3=shutdown, 4=reset, 5=restore factory
}

/// 0x8106: Query terminal attributes
pub fn build_query_attributes() -> Vec<u8> {
    vec![]
}

/// 0x8100: Terminal register response body.
/// Layout:
///   2 bytes reply_serial | 1 byte result | N bytes auth_code
/// Result codes:
///   0 = success
///   1 = vehicle already registered
///   2 = no such vehicle in DB
///   3 = terminal already registered
///   4 = no such terminal in DB
pub fn build_register_response_body(register_serial: u16, result: u8, auth_code: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&register_serial.to_be_bytes());
    body.push(result);
    let code_bytes = auth_code.as_bytes();
    if code_bytes.len() > 255 {
        // Truncate to 255 bytes max (JT/T 808 limit)
        body.extend_from_slice(&code_bytes[..255]);
    } else {
        body.extend_from_slice(code_bytes);
    }
    body
}

/// 0x8100: Build complete terminal register response frame.
pub fn build_register_response(phone: &str, register_serial: u16, result: u8, auth_code: &str) -> Vec<u8> {
    let body = build_register_response_body(register_serial, result, auth_code);
    build_jt808_frame(0x8100, phone, 0, &body)
}

/// 0x8801: Take photo
pub fn build_take_photo(channel_id: u8, photo_cmd: u16, interval: u16, save_flag: u8,
    resolution: u8, quality: u8, brightness: u8, contrast: u8, saturation: u8, chroma: u8) -> Vec<u8> {
    vec![channel_id, (photo_cmd >> 8) as u8, (photo_cmd & 0xFF) as u8,
         (interval >> 8) as u8, (interval & 0xFF) as u8,
         save_flag, resolution, quality, brightness, contrast, saturation, chroma]
}

/// 0x8802: Media search
pub fn build_media_search(media_type: u8, channel_id: u8, event: u8,
    start_time: &[u8; 6], end_time: &[u8; 6]) -> Vec<u8> {
    let mut body = Vec::with_capacity(15);
    body.push(media_type);
    body.push(channel_id);
    body.push(event);
    body.extend_from_slice(start_time);
    body.extend_from_slice(end_time);
    body
}

/// 0x8803: Media upload
pub fn build_media_upload(media_id: u32, delete_flag: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(5);
    body.extend_from_slice(&media_id.to_be_bytes());
    body.push(delete_flag);
    body
}

/// 0x8801 摄像头立即拍摄命令 —— 用作**录像控制**。
///
/// JT/T 808-2019 §8.28 的「拍摄命令」字段取 `0x0001` 表示开始录像、
/// `0x0000` 表示停止录像，因此同一原语即可承载 record start/stop。
///
/// - `duration_secs`：录像时长（秒）；`0` 表示按终端最小间隔持续录像
/// - `save`：`true` 保存到终端存储，`false` 实时上传
pub fn build_record_control(
    channel_id: u8,
    start: bool,
    duration_secs: u16,
    save: bool,
) -> Vec<u8> {
    build_take_photo(
        channel_id,
        if start { 0x0001 } else { 0x0000 },
        duration_secs,
        if save { 1 } else { 0 },
        0x02,
        0x05,
        0x80,
        0x80,
        0x80,
        0x80,
    )
}

/// 0x8202 临时位置跟踪控制
///
/// JT/T 808-2019 §8.16：时间间隔（WORD，秒）+ 位置跟踪有效期（DWORD，秒）。
pub fn build_temp_position_tracking(interval_secs: u16, validity_secs: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(6);
    body.extend_from_slice(&interval_secs.to_be_bytes());
    body.extend_from_slice(&validity_secs.to_be_bytes());
    body
}

/// 0x8203 人工确认报警消息
///
/// JT/T 808-2019 §8.17：报警消息流水号（WORD）+ 人工确认报警类型（DWORD 位标志）。
pub fn build_confirm_alarm(alarm_seq: u16, alarm_type: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(6);
    body.extend_from_slice(&alarm_seq.to_be_bytes());
    body.extend_from_slice(&alarm_type.to_be_bytes());
    body
}

/// 0x9205 文件上传指令（JT/T 1078）
///
/// 请求终端把指定时间段的音视频资源上传到平台 —— 即「录像下载」。
///
/// 字段顺序（JT/T 1078-2016 §5.5）：
/// 音视频资源类型(BYTE) 通道ID(BYTE) 开始时间(BCD[6]) 结束时间(BCD[6])
/// 报警标志(DWORD) 音视频资源掩码(DWORD) 存储器类型(BYTE) 上传方式(BYTE) 最大文件大小(DWORD)
#[allow(clippy::too_many_arguments)]
pub fn build_file_upload_request(
    resource_type: u8,
    channel_id: u8,
    start_time: &[u8; 6],
    end_time: &[u8; 6],
    alarm_flag: u32,
    resource_mask: u32,
    storage_type: u8,
    upload_mode: u8,
    max_file_size: u32,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(32);
    body.push(resource_type);
    body.push(channel_id);
    body.extend_from_slice(start_time);
    body.extend_from_slice(end_time);
    body.extend_from_slice(&alarm_flag.to_be_bytes());
    body.extend_from_slice(&resource_mask.to_be_bytes());
    body.push(storage_type);
    body.push(upload_mode);
    body.extend_from_slice(&max_file_size.to_be_bytes());
    body
}

/// 0x8401: Set phone book
pub fn build_set_phone_book(contacts: &[(String, String)]) -> Vec<u8> {
    let mut body = Vec::new();
    body.push(contacts.len() as u8);
    for (name, phone) in contacts {
        let name_bytes = name.as_bytes();
        body.push(name_bytes.len() as u8);
        body.extend_from_slice(name_bytes);
        let phone_bcd = phone_to_bcd(phone);
        body.extend_from_slice(&phone_bcd);
    }
    body
}

/// 0x8A00: Platform RSA public key (empty for now)
pub fn build_platform_rsa() -> Vec<u8> {
    vec![]
}

/// 0x8203: Manual location report request trigger
pub fn build_manual_location_trigger() -> Vec<u8> {
    vec![]
}

/// 0x8600: Set circular area (geofence)
pub fn build_set_circular_area(_areas: &[(u32, f64, f64, u32, u32, u8)]) -> Vec<u8> {
    // Simple implementation: only supports single area for now
    let mut body = Vec::new();
    body.push(1u8); // count
    body.push(0x01); // set action
    // Return basic frame - full implementation would iterate areas and encode each
    body
}

/// 0x8600: Full implementation with area iteration (kept for future use)

/// 0x8700: Driving record data upload command
pub fn build_driving_record_upload(cmd: u8, data: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(1 + data.len());
    body.push(cmd);
    body.extend_from_slice(data);
    body
}

/// 0x8107: Query terminal properties
pub fn build_query_terminal_properties() -> Vec<u8> {
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_jt808_frame() {
        let frame = build_jt808_frame(0x8103, "13812345678", 1, &[0x01, 0x02]);
        assert_eq!(frame[0], 0x7E);
        assert_eq!(*frame.last().unwrap(), 0x7E);
        // Should have msg_id at positions 1-2
        assert_eq!(u16::from_be_bytes([frame[1], frame[2]]), 0x8103);
    }

    #[test]
    fn test_ptz_direction() {
        let (b1, b2, h, v) = ptz_direction_bytes("UP", 5);
        assert_eq!(b1, 0x05);
        assert_eq!(b2, 0x01);
        assert_eq!(h, 5);
        assert_eq!(v, 0);
    }

    #[test]
    fn test_phone_bcd_roundtrip() {
        let bcd = phone_to_bcd("13812345678");
        let phone = bcd_to_phone(&bcd);
        assert_eq!(phone, "13812345678");
    }

    #[test]
    fn test_build_register_response_body_success() {
        let body = build_register_response_body(42, 0, "AUTH123");
        assert_eq!(body.len(), 2 + 1 + 7);
        assert_eq!(u16::from_be_bytes([body[0], body[1]]), 42);
        assert_eq!(body[2], 0);
        assert_eq!(&body[3..], b"AUTH123");
    }

    #[test]
    fn test_build_register_response_body_failed() {
        let body = build_register_response_body(7, 2, "");
        assert_eq!(body.len(), 3);
        assert_eq!(u16::from_be_bytes([body[0], body[1]]), 7);
        assert_eq!(body[2], 2);
    }

    #[test]
    fn test_build_register_response_frame() {
        let frame = build_register_response("13812345678", 100, 0, "XYZ");
        assert_eq!(frame[0], 0x7E);
        assert_eq!(*frame.last().unwrap(), 0x7E);
        assert_eq!(u16::from_be_bytes([frame[1], frame[2]]), 0x8100);
    }

    // ====== 2026-09-11 补齐的协议原语（用于打通此前"未实现"的 5 个端点）======

    #[test]
    fn test_build_record_control_start_vs_stop() {
        let start = build_record_control(1, true, 0, true);
        let stop = build_record_control(1, false, 0, true);
        assert_eq!(start.len(), 12, "0x8801 体固定 12 字节");
        assert_eq!(start[0], 1, "通道ID");
        // 「拍摄命令」字段为 WORD 大端：1=开始录像 / 0=停止录像
        assert_eq!(u16::from_be_bytes([start[1], start[2]]), 0x0001);
        assert_eq!(u16::from_be_bytes([stop[1], stop[2]]), 0x0000);
        assert_eq!(start[5], 1, "save=true → 保存标志 1");
        assert_eq!(build_record_control(1, true, 0, false)[5], 0, "save=false → 0");
    }

    #[test]
    fn test_build_temp_position_tracking_layout() {
        let body = build_temp_position_tracking(30, 600);
        assert_eq!(body.len(), 6, "WORD(2) + DWORD(4)");
        assert_eq!(u16::from_be_bytes([body[0], body[1]]), 30);
        assert_eq!(
            u32::from_be_bytes([body[2], body[3], body[4], body[5]]),
            600
        );
    }

    #[test]
    fn test_build_confirm_alarm_layout() {
        let body = build_confirm_alarm(0x1234, 0xDEAD_BEEF);
        assert_eq!(body.len(), 6, "WORD(2) + DWORD(4)");
        assert_eq!(u16::from_be_bytes([body[0], body[1]]), 0x1234);
        assert_eq!(
            u32::from_be_bytes([body[2], body[3], body[4], body[5]]),
            0xDEAD_BEEF
        );
    }

    #[test]
    fn test_build_file_upload_request_layout() {
        let st = encode_time_bcd("2026-01-02 03:04:05");
        let et = encode_time_bcd("2026-01-02 04:05:06");
        let body = build_file_upload_request(0, 3, &st, &et, 0, 0xFFFF_FFFF, 0, 1, 0);
        // 1 + 1 + 6 + 6 + 4 + 4 + 1 + 1 + 4 = 28
        assert_eq!(body.len(), 28, "0x9205 体固定 28 字节");
        assert_eq!(body[0], 0, "资源类型=音视频");
        assert_eq!(body[1], 3, "通道ID");
        assert_eq!(&body[2..8], &st[..], "开始时间 BCD");
        assert_eq!(&body[8..14], &et[..], "结束时间 BCD");
        assert_eq!(
            u32::from_be_bytes([body[24], body[25], body[26], body[27]]),
            0,
            "最大文件大小=0 表示不限制"
        );
    }

    #[test]
    fn test_try_encode_time_bcd_accepts_supported_formats() {
        // **真 BCD**：2026-01-02 03:04:05 → 0x26 0x01 0x02 0x03 0x04 0x05
        let expected = [0x26u8, 0x01, 0x02, 0x03, 0x04, 0x05];
        assert_eq!(try_encode_time_bcd("2026-01-02 03:04:05"), Some(expected));
        assert_eq!(try_encode_time_bcd("2026-01-02T03:04:05"), Some(expected));
        assert_eq!(try_encode_time_bcd("2026-01-02"), Some([0x26, 0x01, 0x02, 0, 0, 0]));
        // Unix 时间戳（秒）
        assert!(try_encode_time_bcd("1767323045").is_some());
        // 毫秒时间戳
        assert!(try_encode_time_bcd("1767323045000").is_some());
    }

    /// 关键差异：严格版本必须拒绝非法输入，而 `encode_time_bcd` 会**静默回退到当前时间**。
    /// 对「录像下载」来说后者会把错误时间段下发到终端。
    #[test]
    fn test_try_encode_time_bcd_rejects_invalid_while_lenient_falls_back() {
        for bad in ["", "   ", "not-a-time", "2026-13-45 99:99:99"] {
            assert!(
                try_encode_time_bcd(bad).is_none(),
                "严格解析必须拒绝 {:?}",
                bad
            );
        }
        // 宽松版本对同样输入不报错（这正是需要严格版本的原因）
        let _ = encode_time_bcd("not-a-time");
    }
}
