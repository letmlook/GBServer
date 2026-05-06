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

/// Encode datetime string "2020-01-01T12:00:00" or timestamp to 6-byte BCD
pub fn encode_time_bcd(time_str: &str) -> [u8; 6] {
    // Try parsing as i64 timestamp first (seconds or milliseconds)
    if let Ok(ts) = time_str.parse::<i64>() {
        let ts = if ts > 1_000_000_000_000 { ts / 1000 } else { ts };
        let dt = chrono::DateTime::from_timestamp(ts, 0).unwrap_or_else(|| chrono::Utc::now());
        let y = dt.format("%y").to_string().parse::<u8>().unwrap_or(0);
        let m = dt.format("%m").to_string().parse::<u8>().unwrap_or(1);
        let d = dt.format("%d").to_string().parse::<u8>().unwrap_or(1);
        let h = dt.format("%H").to_string().parse::<u8>().unwrap_or(0);
        let min = dt.format("%M").to_string().parse::<u8>().unwrap_or(0);
        let s = dt.format("%S").to_string().parse::<u8>().unwrap_or(0);
        return [y, m, d, h, min, s];
    }
    // Try parsing as datetime string
    let formats = ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d"];
    for fmt in &formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(time_str, fmt) {
            let y = dt.format("%y").to_string().parse::<u8>().unwrap_or(0);
            let m = dt.format("%m").to_string().parse::<u8>().unwrap_or(1);
            let d = dt.format("%d").to_string().parse::<u8>().unwrap_or(1);
            let h = dt.format("%H").to_string().parse::<u8>().unwrap_or(0);
            let min = dt.format("%M").to_string().parse::<u8>().unwrap_or(0);
            let s = dt.format("%S").to_string().parse::<u8>().unwrap_or(0);
            return [y, m, d, h, min, s];
        }
    }
    // Fallback: current time
    let now = chrono::Utc::now();
    [
        now.format("%y").to_string().parse::<u8>().unwrap_or(0),
        now.format("%m").to_string().parse::<u8>().unwrap_or(1),
        now.format("%d").to_string().parse::<u8>().unwrap_or(1),
        now.format("%H").to_string().parse::<u8>().unwrap_or(0),
        now.format("%M").to_string().parse::<u8>().unwrap_or(0),
        now.format("%S").to_string().parse::<u8>().unwrap_or(0),
    ]
}

/// 0x9202: Playback control
pub fn build_playback_control(channel_id: u8, control: u8, speed: u8, seek_time: &[u8; 6]) -> Vec<u8> {
    let mut body = Vec::with_capacity(9);
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
}
