//! JT/T 808 response parsers — parse inbound business messages from terminal
//!
//! Reference: JT/T 808-2011 / JT/T 1078-2016 道路运输车辆卫星定位系统终端通讯协议
//!
//! Conventions:
//! - BCD-encoded digits: each byte carries two decimal digits (high nibble = tens, low nibble = ones)
//! - Strings: zero/space padded, ASCII
//! - All multi-byte integers are big-endian
//!
//! Phases:
//! - Phase 6.1: terminal register 0x0100
//! - Phase 6.4: media items first 0x0801, location report 0x0200, attribute report 0x0102
//! - Phase 6.5: query terminal params response 0x0107

use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, NaiveTime, Utc};

/// Terminal register request 0x0100
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterRequest {
    pub province_id: u16,
    pub city_id: u16,
    pub manufacturer: String,  // 5 bytes ASCII
    pub terminal_model: String, // 20 bytes ASCII
    pub terminal_id: String,    // 7 bytes ASCII
    pub iccid: String,          // 10 bytes BCD
    pub hardware_version: String, // variable length
}

const REGISTER_MIN_LEN: usize = 2 + 2 + 5 + 20 + 7 + 10; // = 46

/// Parse 0x0100 terminal register body.
/// Layout (JT/T 808 §4.5.1.1):
///   2 bytes province_id | 2 bytes city_id | 5 bytes manufacturer
///   20 bytes terminal_model | 7 bytes terminal_id | 10 bytes iccid (BCD)
///   1 byte hardware_version_length | N bytes hardware_version
pub fn parse_register_request(body: &[u8]) -> Result<RegisterRequest, String> {
    if body.len() < REGISTER_MIN_LEN {
        return Err(format!(
            "register body too short: got {} bytes, need at least {}",
            body.len(),
            REGISTER_MIN_LEN
        ));
    }
    let province_id = u16::from_be_bytes([body[0], body[1]]);
    let city_id = u16::from_be_bytes([body[2], body[3]]);
    let manufacturer = read_ascii_field(&body[4..9])?;
    let terminal_model = read_ascii_field(&body[9..29])?;
    let terminal_id = read_ascii_field(&body[29..36])?;
    let iccid = read_bcd_field(&body[36..46])?;
    let hardware_version = if body.len() > 46 {
        // Optional hardware_version with leading length byte
        let len = body[46] as usize;
        if 47 + len <= body.len() {
            read_ascii_field(&body[47..(47 + len)])?
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    Ok(RegisterRequest {
        province_id,
        city_id,
        manufacturer,
        terminal_model,
        terminal_id,
        iccid,
        hardware_version,
    })
}

/// Location report 0x0200 (basic, 28 bytes)
#[derive(Debug, Clone)]
pub struct LocationReport {
    pub alarm: u32,
    pub status: u32,
    pub latitude: f64,   // raw / 1_000_000
    pub longitude: f64,  // raw / 1_000_000
    pub altitude: u16,
    pub speed: u16,
    pub direction: u16,
    pub time: DateTime<Utc>,
}

const LOCATION_BASIC_LEN: usize = 28;

/// Parse 0x0200 location report body (basic 28-byte form).
pub fn parse_location_report(body: &[u8]) -> Result<LocationReport, String> {
    if body.len() < LOCATION_BASIC_LEN {
        return Err(format!(
            "location report too short: got {}, need {}",
            body.len(),
            LOCATION_BASIC_LEN
        ));
    }
    let alarm = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
    let status = u32::from_be_bytes([body[4], body[5], body[6], body[7]]);
    let lat_raw = u32::from_be_bytes([body[8], body[9], body[10], body[11]]);
    let lng_raw = u32::from_be_bytes([body[12], body[13], body[14], body[15]]);
    let altitude = u16::from_be_bytes([body[16], body[17]]);
    let speed = u16::from_be_bytes([body[18], body[19]]);
    let direction = u16::from_be_bytes([body[20], body[21]]);
    let time = parse_bcd_datetime(&body[22..28])?;
    Ok(LocationReport {
        alarm,
        status,
        latitude: lat_raw as f64 / 1_000_000.0,
        longitude: lng_raw as f64 / 1_000_000.0,
        altitude,
        speed,
        direction,
        time,
    })
}

/// Terminal attribute report 0x0102
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeReport {
    pub terminal_type: u16,
    pub maker_id: String,       // 5 bytes ASCII
    pub terminal_model: String, // 20 bytes ASCII
    pub terminal_id: String,    // 7 bytes ASCII
    pub iccid: String,          // 10 bytes BCD
    pub hardware_version: String,
    pub firmware_version: String,
}

const ATTR_MIN_LEN: usize = 2 + 5 + 20 + 7 + 10; // = 44

/// Parse 0x0102 terminal attribute report body.
pub fn parse_attribute_report(body: &[u8]) -> Result<AttributeReport, String> {
    if body.len() < ATTR_MIN_LEN {
        return Err(format!(
            "attribute report too short: got {}, need {}",
            body.len(),
            ATTR_MIN_LEN
        ));
    }
    let terminal_type = u16::from_be_bytes([body[0], body[1]]);
    let maker_id = read_ascii_field(&body[2..7])?;
    let terminal_model = read_ascii_field(&body[7..27])?;
    let terminal_id = read_ascii_field(&body[27..34])?;
    let iccid = read_bcd_field(&body[34..44])?;
    let mut pos = 44;
    let (hardware_version, p1) = read_length_prefixed_ascii(body, pos)?;
    pos = p1;
    let (firmware_version, _) = read_length_prefixed_ascii(body, pos)?;
    Ok(AttributeReport {
        terminal_type,
        maker_id,
        terminal_model,
        terminal_id,
        iccid,
        hardware_version,
        firmware_version,
    })
}

/// One item from media retrieval 0x0801 (Phase 6.4 partial)
#[derive(Debug, Clone)]
pub struct MediaItem {
    pub media_id: u32,
    pub media_type: u8,
    pub media_format: u8,
    pub channel_id: u8,
    pub event_code: u8,
    pub position: Option<LocationReport>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}

/// Parse 0x0801 first frame body (1 item per packet per JT/T 1078 §7.4.4).
/// Layout (9 bytes header + N * item):
///   4 bytes media_id | 1 byte media_type | 1 byte media_format | 1 byte channel_id
///   1 byte event_code | 1 byte location_flag | [optional location 28 bytes]
///   6 bytes start_time (BCD) | 6 bytes end_time (BCD)
pub fn parse_media_item_first(body: &[u8]) -> Result<MediaItem, String> {
    if body.len() < 21 {
        return Err(format!(
            "media item too short: got {}, need at least 21",
            body.len()
        ));
    }
    let media_id = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
    let media_type = body[4];
    let media_format = body[5];
    let channel_id = body[6];
    let event_code = body[7];
    // 1 byte location flag (0 = has location, 1 = no location)
    let has_location = body[8] == 0;
    let pos_offset = 9;
    let position = if has_location && body.len() >= pos_offset + 28 {
        Some(parse_location_report(&body[pos_offset..(pos_offset + 28)])?)
    } else {
        None
    };
    let time_start = if has_location {
        pos_offset + 28
    } else {
        pos_offset
    };
    if body.len() < time_start + 12 {
        return Err("media item missing time range".to_string());
    }
    let start_time = parse_bcd_datetime(&body[time_start..(time_start + 6)])?;
    let end_time = parse_bcd_datetime(&body[(time_start + 6)..(time_start + 12)])?;
    Ok(MediaItem {
        media_id,
        media_type,
        media_format,
        channel_id,
        event_code,
        position,
        start_time,
        end_time,
    })
}

/// Query terminal params response 0x0107 (Phase 6.5)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalParam {
    pub param_id: u32,
    pub param_length: u8,
    pub param_value: Vec<u8>,
}

/// Parse 0x0107 query terminal params response body — list of (id, len, value) entries.
pub fn parse_query_params_response(body: &[u8]) -> Result<Vec<TerminalParam>, String> {
    let mut params = Vec::new();
    let mut pos = 0;
    while pos < body.len() {
        if pos + 5 > body.len() {
            return Err("truncated param header".to_string());
        }
        let param_id = u32::from_be_bytes([body[pos], body[pos + 1], body[pos + 2], body[pos + 3]]);
        let param_length = body[pos + 4];
        pos += 5;
        if pos + param_length as usize > body.len() {
            return Err("truncated param value".to_string());
        }
        let param_value = body[pos..(pos + param_length as usize)].to_vec();
        pos += param_length as usize;
        params.push(TerminalParam {
            param_id,
            param_length,
            param_value,
        });
    }
    Ok(params)
}

// ============== Internal helpers ==============

fn read_ascii_field(bytes: &[u8]) -> Result<String, String> {
    Ok(std::str::from_utf8(bytes)
        .map_err(|e| format!("invalid utf-8 in ASCII field: {}", e))?
        .trim_end_matches('\0')
        .trim()
        .to_string())
}

fn read_bcd_field(bcd: &[u8]) -> Result<String, String> {
    let mut s = String::with_capacity(bcd.len() * 2);
    for &b in bcd {
        s.push(decimal_nibble(b >> 4)?);
        s.push(decimal_nibble(b & 0x0F)?);
    }
    Ok(s)
}

fn decimal_nibble(n: u8) -> Result<char, String> {
    if n > 9 {
        Err(format!("invalid BCD nibble: 0x{:x}", n))
    } else {
        Ok((b'0' + n) as char)
    }
}

fn parse_bcd_datetime(bcd: &[u8]) -> Result<DateTime<Utc>, String> {
    if bcd.len() != 6 {
        return Err(format!("BCD datetime must be 6 bytes, got {}", bcd.len()));
    }
    let s = read_bcd_field(bcd)?;
    // Format: YY MM DD HH mm ss (each 2 digits)
    if s.len() != 12 {
        return Err(format!("BCD datetime string length: {}", s.len()));
    }
    let yy: i32 = s[0..2].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let mm: u32 = s[2..4].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let dd: u32 = s[4..6].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let hh: u32 = s[6..8].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let mi: u32 = s[8..10].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let ss: u32 = s[10..12].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let year = if yy >= 70 { 1900 + yy } else { 2000 + yy };
    let date = NaiveDate::from_ymd_opt(year, mm, dd)
        .ok_or_else(|| format!("invalid date Y={} M={} D={}", year, mm, dd))?;
    let time = NaiveTime::from_hms_opt(hh, mi, ss)
        .ok_or_else(|| format!("invalid time {}:{}:{}", hh, mi, ss))?;
    let dt = NaiveDateTime::new(date, time);
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
}

fn read_length_prefixed_ascii(body: &[u8], pos: usize) -> Result<(String, usize), String> {
    if pos >= body.len() {
        return Ok((String::new(), pos));
    }
    let len = body[pos] as usize;
    let new_pos = pos + 1;
    if new_pos + len > body.len() {
        return Err(format!(
            "length-prefixed field overflow: pos={} len={} body_len={}",
            pos,
            len,
            body.len()
        ));
    }
    let s = read_ascii_field(&body[new_pos..(new_pos + len)])?;
    Ok((s, new_pos + len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bcd_to_string_full() {
        let bcd = [0x12u8, 0x34, 0x56, 0x78, 0x90];
        let s = read_bcd_field(&bcd).unwrap();
        assert_eq!(s, "1234567890");
    }

    #[test]
    fn test_bcd_to_string_with_nibbles() {
        let bcd = [0x00u8, 0x01, 0x09, 0x10];
        let s = read_bcd_field(&bcd).unwrap();
        assert_eq!(s, "00010910");
    }

    #[test]
    fn test_bcd_to_string_invalid() {
        let bcd = [0xFFu8, 0x00];
        assert!(read_bcd_field(&bcd).is_err());
    }

    fn build_register_body() -> Vec<u8> {
        // 2+2+5+20+7+10 = 46 minimum, no hardware_version
        let mut b = Vec::new();
        b.extend_from_slice(&0x0100u16.to_be_bytes()); // province
        b.extend_from_slice(&0x0200u16.to_be_bytes()); // city
        b.extend_from_slice(b"AAAAA");                // manufacturer
        b.extend_from_slice(&vec![b'B'; 20]);          // terminal_model
        b.extend_from_slice(b"1234567");              // terminal_id
        b.extend_from_slice(&[0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90]); // ICCID
        b
    }

    #[test]
    fn test_parse_register_request_valid() {
        let body = build_register_body();
        let req = parse_register_request(&body).unwrap();
        assert_eq!(req.province_id, 0x0100);
        assert_eq!(req.city_id, 0x0200);
        assert_eq!(req.manufacturer, "AAAAA");
        assert_eq!(req.terminal_model, "B".repeat(20).trim_end().to_string());
        assert_eq!(req.terminal_id, "1234567");
        assert_eq!(req.iccid, "12345678901234567890");
        assert_eq!(req.hardware_version, "");
    }

    #[test]
    fn test_parse_register_request_with_hw_version() {
        let mut body = build_register_body();
        body.push(5u8); // length 5
        body.extend_from_slice(b"v1.00");
        let req = parse_register_request(&body).unwrap();
        assert_eq!(req.hardware_version, "v1.00");
    }

    #[test]
    fn test_parse_register_request_too_short() {
        let body = vec![0u8; 30];
        assert!(parse_register_request(&body).is_err());
    }

    #[test]
    fn test_parse_location_report_valid() {
        let mut body = vec![0u8; 28];
        body[0..4].copy_from_slice(&0x12345678u32.to_be_bytes());
        body[4..8].copy_from_slice(&0xAABBCCDDu32.to_be_bytes());
        body[8..12].copy_from_slice(&39_904_321u32.to_be_bytes()); // lat
        body[12..16].copy_from_slice(&116_407_215u32.to_be_bytes()); // lng
        body[16..18].copy_from_slice(&50u16.to_be_bytes());
        body[18..20].copy_from_slice(&120u16.to_be_bytes());
        body[20..22].copy_from_slice(&270u16.to_be_bytes());
        // BCD time: 26 06 20 14 30 00
        body[22..28].copy_from_slice(&[0x26, 0x06, 0x20, 0x14, 0x30, 0x00]);
        let loc = parse_location_report(&body).unwrap();
        assert_eq!(loc.alarm, 0x12345678);
        assert_eq!(loc.latitude, 39.904_321);
        assert_eq!(loc.longitude, 116.407_215);
        assert_eq!(loc.altitude, 50);
        assert_eq!(loc.speed, 120);
        assert_eq!(loc.direction, 270);
        assert_eq!(loc.time.year(), 2026);
        assert_eq!(loc.time.month(), 6);
        assert_eq!(loc.time.day(), 20);
    }

    #[test]
    fn test_parse_location_report_too_short() {
        let body = vec![0u8; 20];
        assert!(parse_location_report(&body).is_err());
    }

    #[test]
    fn test_parse_attribute_report_valid() {
        // 2 + 5 + 20 + 7 + 10 = 44 bytes base
        // + 1 byte hw_len + "v1.0" (4 bytes) + 1 byte fw_len + "f1" (2 bytes)
        let mut body = Vec::new();
        body.extend_from_slice(&0x0001u16.to_be_bytes());
        body.extend_from_slice(b"AAAAA");
        body.extend_from_slice(&vec![b'B'; 20]);
        body.extend_from_slice(b"1234567");
        body.extend_from_slice(&[0u8; 10]); // ICCID zeros
        body.push(4u8);
        body.extend_from_slice(b"v1.0");
        body.push(2u8);
        body.extend_from_slice(b"f1");
        let attr = parse_attribute_report(&body).unwrap();
        assert_eq!(attr.terminal_type, 1);
        assert_eq!(attr.maker_id, "AAAAA");
        assert_eq!(attr.terminal_id, "1234567");
        assert_eq!(attr.hardware_version, "v1.0");
        assert_eq!(attr.firmware_version, "f1");
    }

    #[test]
    fn test_parse_attribute_report_too_short() {
        let body = vec![0u8; 30];
        assert!(parse_attribute_report(&body).is_err());
    }

    #[test]
    fn test_parse_media_item_first_valid() {
        // 9-byte header: media_id(4) type(1) format(1) ch(1) event(1) location_flag(1)
        // Then 6 bytes start + 6 bytes end
        let mut body = Vec::new();
        body.extend_from_slice(&1234u32.to_be_bytes());
        body.push(0); // media_type
        body.push(0); // media_format
        body.push(1); // channel_id
        body.push(0); // event_code
        body.push(1); // location_flag = 1 (no location)
        body.extend_from_slice(&[0x26, 0x06, 0x20, 0x14, 0x30, 0x00]); // start BCD
        body.extend_from_slice(&[0x26, 0x06, 0x20, 0x15, 0x30, 0x00]); // end BCD
        let item = parse_media_item_first(&body).unwrap();
        assert_eq!(item.media_id, 1234);
        assert_eq!(item.channel_id, 1);
        assert!(item.position.is_none());
        assert_eq!(item.start_time.year(), 2026);
    }

    #[test]
    fn test_parse_query_params_response_valid() {
        // One entry: param_id=0x0010, length=4, value=0x12345678
        let mut body = Vec::new();
        body.extend_from_slice(&0x0010u32.to_be_bytes());
        body.push(4);
        body.extend_from_slice(&0x12345678u32.to_be_bytes());
        let params = parse_query_params_response(&body).unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].param_id, 0x10);
        assert_eq!(params[0].param_length, 4);
        assert_eq!(params[0].param_value, vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_parse_query_params_response_empty() {
        let body: Vec<u8> = vec![];
        let params = parse_query_params_response(&body).unwrap();
        assert!(params.is_empty());
    }

    #[test]
    fn test_parse_query_params_response_truncated_header() {
        let body = vec![0u8; 3];
        assert!(parse_query_params_response(&body).is_err());
    }
}
