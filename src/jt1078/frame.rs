// JT1078 frame parsing helpers
// Supports two common frame boundary styles:
// 1) Magic start-marker 0x7E 0x01 followed by a 2-byte big-endian length (header = 4 bytes)
// 2) Fallback: 4-byte big-endian length prefix (header = 4 bytes)

/// Parse a JT1078 frame from the provided buffer.
/// Returns Some((total_frame_len, payload_slice)) when a complete frame is present,
/// otherwise returns None to indicate more bytes are needed.
pub fn parse_jt1078_frame(buf: &[u8]) -> Option<(usize, &[u8])> {
    // Mode A: Magic marker 0x7E 0x01, length in next 2 bytes (u16 BE)
    if buf.len() >= 4 && buf[0] == 0x7E && buf[1] == 0x01 {
        let len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_frame_length_prefix() {
        let payload = b"hello-jt1078";
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
    fn test_parse_partial_frame_length_prefix() {
        let payload = b"abc";
        let len = (payload.len() as u32).to_be_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&len);
        buf.extend_from_slice(&payload[..1]);

        let res = parse_jt1078_frame(&buf);
        assert!(res.is_none());
    }

    #[test]
    fn test_parse_magic_header_frame() {
        // Build frame with magic header 0x7E 0x01 and 2-byte length
        let payload = b"magic-frame-payload";
        let len = (payload.len() as u16).to_be_bytes();
        let mut buf = Vec::new();
        buf.push(0x7E);
        buf.push(0x01);
        buf.extend_from_slice(&len);
        buf.extend_from_slice(payload);

        let res = parse_jt1078_frame(&buf);
        assert!(res.is_some());
        let (frame_len, p) = res.unwrap();
        assert_eq!(frame_len, 4 + payload.len());
        assert_eq!(p, payload);
    }

    #[test]
    fn test_parse_magic_header_partial() {
        // Magic header but incomplete payload
        let payload = b"xyz";
        let len = (payload.len() as u16).to_be_bytes();
        let mut buf = Vec::new();
        buf.push(0x7E);
        buf.push(0x01);
        buf.extend_from_slice(&len);
        buf.extend_from_slice(&payload[..1]);

        let res = parse_jt1078_frame(&buf);
        assert!(res.is_none());
    }
}
