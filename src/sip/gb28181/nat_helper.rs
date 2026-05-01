pub struct NatHelper {
    sdp_ip: Option<String>,
    stream_ip: Option<String>,
    local_ip: String,
}

impl NatHelper {
    pub fn new(local_ip: &str, sdp_ip: Option<&str>, stream_ip: Option<&str>) -> Self {
        Self {
            sdp_ip: sdp_ip.map(|s| s.to_string()),
            stream_ip: stream_ip.map(|s| s.to_string()),
            local_ip: local_ip.to_string(),
        }
    }

    pub fn resolve_sdp_ip(&self) -> &str {
        self.sdp_ip.as_deref().unwrap_or(&self.local_ip)
    }

    pub fn resolve_stream_ip(&self) -> &str {
        self.stream_ip.as_deref().unwrap_or(&self.local_ip)
    }

    pub fn get_device_real_addr(&self, via_received: Option<&str>, via_rport: Option<u16>, contact_ip: Option<&str>, contact_port: Option<u16>) -> (String, u16) {
        let ip = via_received
            .or(contact_ip)
            .unwrap_or(&self.local_ip);
        let port = via_rport
            .or(contact_port)
            .unwrap_or(5060);
        (ip.to_string(), port)
    }

    pub fn replace_sdp_ip(&self, sdp: &str) -> String {
        if let Some(ref sdp_ip) = self.sdp_ip {
            sdp.replace(&self.local_ip, sdp_ip)
        } else {
            sdp.to_string()
        }
    }

    pub fn sdp_ip_configured(&self) -> bool {
        self.sdp_ip.is_some()
    }

    pub fn stream_ip_configured(&self) -> bool {
        self.stream_ip.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_sdp_ip_configured() {
        let helper = NatHelper::new("192.168.1.100", Some("1.2.3.4"), None);
        assert_eq!(helper.resolve_sdp_ip(), "1.2.3.4");
    }

    #[test]
    fn test_resolve_sdp_ip_fallback() {
        let helper = NatHelper::new("192.168.1.100", None, None);
        assert_eq!(helper.resolve_sdp_ip(), "192.168.1.100");
    }

    #[test]
    fn test_resolve_stream_ip() {
        let helper = NatHelper::new("192.168.1.100", None, Some("5.6.7.8"));
        assert_eq!(helper.resolve_stream_ip(), "5.6.7.8");
    }

    #[test]
    fn test_get_device_real_addr() {
        let helper = NatHelper::new("192.168.1.100", None, None);
        let (ip, port) = helper.get_device_real_addr(Some("10.0.0.5"), Some(5062), None, None);
        assert_eq!(ip, "10.0.0.5");
        assert_eq!(port, 5062);
    }

    #[test]
    fn test_get_device_real_addr_fallback() {
        let helper = NatHelper::new("192.168.1.100", None, None);
        let (ip, port) = helper.get_device_real_addr(None, None, Some("10.0.0.5"), Some(5080));
        assert_eq!(ip, "10.0.0.5");
        assert_eq!(port, 5080);
    }

    #[test]
    fn test_replace_sdp_ip() {
        let helper = NatHelper::new("192.168.1.100", Some("1.2.3.4"), None);
        let sdp = "c=IN IP4 192.168.1.100\r\nm=video 5000 RTP/AVP 96\r\n";
        let replaced = helper.replace_sdp_ip(sdp);
        assert!(replaced.contains("1.2.3.4"));
        assert!(!replaced.contains("192.168.1.100"));
    }

    #[test]
    fn test_replace_sdp_ip_no_config() {
        let helper = NatHelper::new("192.168.1.100", None, None);
        let sdp = "c=IN IP4 192.168.1.100\r\n";
        assert_eq!(helper.replace_sdp_ip(sdp), sdp);
    }
}
