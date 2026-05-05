// JT1078 frame parsing helpers
// Supports two common frame boundary styles:
// 1) Magic start-marker 0x7E 0x01 followed by a 2-byte big-endian payload length and header fields
// 2) Fallback: 4-byte big-endian length prefix (legacy/demo)

/// Simple legacy parser (keeps backward compatibility): returns payload slice when full frame available.
pub fn parse_jt1078_frame(buf: &[u8]) -> Option<(usize, &[u8])> {
    // Mode A: Magic marker 0x7E 0x01, length in next 2 bytes (u16 BE) — here len means payload length only
    if buf.len() >= 4 && buf[0] == 0x7E && buf[1] == 0x01 {
        let len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
        // For legacy compatibility we treat the payload as immediately following
        if buf.len() >= 4 + len {
            return Some((4 + len, &buf[4..4 + len]));
        } else {
            return None;
        }
    }

    // Mode B: 4-byte big-endian length prefix (legacy / demo mode)
    if buf.len() < 4 {
        return None;
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() >= 4 + len {
        Some((4 + len, &buf[4..4 + len]))
    } else {
        None
    }
}

/// Structured JT1078 frame representation (example fields).
/// Note: JT/T1078 official spec is more complex; this implementation provides a practical
/// header for reassembly, sequencing, timestamping and checksum verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jt1078Frame {
    pub seq: u16,
    pub timestamp: u32,
    pub payload: Vec<u8>,
    pub checksum: u8,
}

/// Parse a structured JT1078 frame with the following example layout:
/// [0x7E][0x01][payload_len: u16 BE][seq: u16 BE][timestamp: u32 BE][payload bytes...][checksum: u8]
/// Returns Some((total_frame_len, Jt1078Frame)) when a complete frame is available and checksum matches.
pub fn parse_jt1078_structured_frame(buf: &[u8]) -> Option<(usize, Jt1078Frame)> {
    if buf.len() < 10 {
        return None; // need at least magic+len+seq+timestamp
    }
    if !(buf[0] == 0x7E && buf[1] == 0x01) {
        return None;
    }
    let payload_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    // header sizes: magic+len (4) + seq (2) + timestamp (4) = 10
    let total_len = 10 + payload_len + 1; // +1 checksum
    if buf.len() < total_len {
        return None;
    }

    let seq = u16::from_be_bytes([buf[4], buf[5]]);
    let timestamp = u32::from_be_bytes([buf[6], buf[7], buf[8], buf[9]]);
    let payload_start = 10;
    let payload_end = payload_start + payload_len;
    let payload = buf[payload_start..payload_end].to_vec();
    let checksum = buf[payload_end];

    // Simple checksum: XOR of payload bytes (example)
    let mut xor: u8 = 0;
    for &b in &payload {
        xor ^= b;
    }

    if xor != checksum {
        return None; // checksum mismatch — drop
    }

    Some((total_len, Jt1078Frame { seq, timestamp, payload, checksum }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_length_prefix() {
        let payload = b"legacy-payload";
        let len = (payload.len() as u32).to_be_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&len);
        buf.extend_from_slice(payload);

        let res = parse_jt1078_frame(&buf);
        assert!(res.is_some());
        let (frame_len, p) = res.unwrap();
        assert_eq!(frame_len, 4 + payload.len());
        assert_eq!(p, payload);
    }

    #[test]
    fn test_structured_frame_ok() {
        let payload = b"hello-structured";
        let payload_len = (payload.len() as u16).to_be_bytes();
        let seq = 0x1234u16.to_be_bytes();
        let timestamp = 0xDEADBEEFu32.to_be_bytes();

        let mut xor: u8 = 0;
        for &b in payload.iter() { xor ^= b; }

        let mut buf = Vec::new();
        buf.push(0x7E);
        buf.push(0x01);
        buf.extend_from_slice(&payload_len);
        buf.extend_from_slice(&seq);
        buf.extend_from_slice(&timestamp);
        buf.extend_from_slice(payload);
        buf.push(xor);

        let res = parse_jt1078_structured_frame(&buf);
        assert!(res.is_some());
        let (total_len, frame) = res.unwrap();
        assert_eq!(total_len, 10 + payload.len() + 1);
        assert_eq!(frame.seq, 0x1234);
        assert_eq!(frame.timestamp, 0xDEADBEEF);
        assert_eq!(frame.payload, payload);
        assert_eq!(frame.checksum, xor);
    }

    #[test]
    fn test_structured_frame_checksum_fail() {
        let payload = b"bad-checksum";
        let payload_len = (payload.len() as u16).to_be_bytes();
        let seq = 1u16.to_be_bytes();
        let timestamp = 0u32.to_be_bytes();
        let mut buf = Vec::new();
        buf.push(0x7E);
        buf.push(0x01);
        buf.extend_from_slice(&payload_len);
        buf.extend_from_slice(&seq);
        buf.extend_from_slice(&timestamp);
        buf.extend_from_slice(payload);
        buf.push(0x00); // incorrect checksum

        let res = parse_jt1078_structured_frame(&buf);
        assert!(res.is_none());
    }
}
