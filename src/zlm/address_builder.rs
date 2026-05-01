use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZlmPortConfig {
    pub http_port: u16,
    pub https_port: Option<u16>,
    pub rtsp_port: u16,
    pub rtsps_port: Option<u16>,
    pub rtmp_port: u16,
    pub rtmps_port: Option<u16>,
    pub flv_port: Option<u16>,
    pub ws_flv_port: Option<u16>,
    pub hls_port: Option<u16>,
    pub webrtc_port: Option<u16>,
}

impl Default for ZlmPortConfig {
    fn default() -> Self {
        Self {
            http_port: 8080,
            https_port: None,
            rtsp_port: 554,
            rtsps_port: None,
            rtmp_port: 1935,
            rtmps_port: None,
            flv_port: Some(8080),
            ws_flv_port: Some(8080),
            hls_port: Some(8080),
            webrtc_port: None,
        }
    }
}

impl ZlmPortConfig {
    pub fn from_server_config(config: &HashMap<String, String>) -> Self {
        let get_port = |key: &str| config.get(key).and_then(|v| v.parse::<u16>().ok());

        Self {
            http_port: get_port("http.port").unwrap_or(8080),
            https_port: get_port("https.port"),
            rtsp_port: get_port("rtsp.port").unwrap_or(554),
            rtsps_port: get_port("rtsps.port"),
            rtmp_port: get_port("rtmp.port").unwrap_or(1935),
            rtmps_port: get_port("rtmps.port"),
            flv_port: get_port("http.port").or_else(|| get_port("flv.port")),
            ws_flv_port: get_port("http.port").or_else(|| get_port("ws.port")),
            hls_port: get_port("http.port").or_else(|| get_port("hls.port")),
            webrtc_port: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamAddresses {
    pub rtsp: String,
    pub rtmp: String,
    pub hls: String,
    pub flv: String,
    pub ws_flv: String,
    pub webrtc: Option<String>,
}

pub struct StreamAddressBuilder {
    ip: String,
    port_config: ZlmPortConfig,
}

impl StreamAddressBuilder {
    pub fn new(ip: &str, port_config: ZlmPortConfig) -> Self {
        Self {
            ip: ip.to_string(),
            port_config,
        }
    }

    pub fn build(&self, app: &str, stream_id: &str) -> StreamAddresses {
        let stream_path = format!("{}/{}", app, stream_id);

        StreamAddresses {
            rtsp: format!("rtsp://{}:{}/{}", self.ip, self.port_config.rtsp_port, stream_path),
            rtmp: format!("rtmp://{}:{}/{}", self.ip, self.port_config.rtmp_port, stream_path),
            hls: format!("http://{}:{}/{}/hls.m3u8", 
                self.ip, 
                self.port_config.hls_port.unwrap_or(self.port_config.http_port), 
                stream_path),
            flv: format!("http://{}:{}/{}.flv", 
                self.ip, 
                self.port_config.flv_port.unwrap_or(self.port_config.http_port), 
                stream_path),
            ws_flv: format!("ws://{}:{}/{}.flv", 
                self.ip, 
                self.port_config.ws_flv_port.unwrap_or(self.port_config.http_port), 
                stream_path),
            webrtc: self.port_config.webrtc_port.map(|port| {
                format!("webrtc://{}:{}/index/api/webrtc?app={}&stream={}&type=play", 
                    self.ip, port, app, stream_id)
            }).or_else(|| Some(format!("webrtc://{}:{}/index/api/webrtc?app={}&stream={}&type=play", 
                self.ip, self.port_config.http_port, app, stream_id))),
        }
    }

    pub fn build_with_webrtc_api(&self, app: &str, stream_id: &str) -> StreamAddresses {
        let mut addrs = self.build(app, stream_id);
        addrs.webrtc = Some(format!("webrtc://{}:{}/index/api/webrtc?app={}&stream={}&type=play", 
            self.ip, self.port_config.http_port, app, stream_id));
        addrs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_addresses() {
        let config = ZlmPortConfig::default();
        let builder = StreamAddressBuilder::new("192.168.1.100", config);
        let addrs = builder.build("rtp", "device1_channel1");

        assert!(addrs.rtsp.starts_with("rtsp://192.168.1.100:554/"));
        assert!(addrs.rtmp.starts_with("rtmp://192.168.1.100:1935/"));
        assert!(addrs.hls.contains("hls.m3u8"));
        assert!(addrs.flv.ends_with(".flv"));
        assert!(addrs.ws_flv.starts_with("ws://"));
        assert!(addrs.webrtc.is_some());
    }

    #[test]
    fn test_custom_ports() {
        let config = ZlmPortConfig {
            http_port: 8888,
            rtsp_port: 8554,
            rtmp_port: 19350,
            ..ZlmPortConfig::default()
        };
        let builder = StreamAddressBuilder::new("10.0.0.1", config);
        let addrs = builder.build("rtp", "test");

        assert!(addrs.rtsp.contains(":8554/"));
        assert!(addrs.rtmp.contains(":19350/"));
    }

    #[test]
    fn test_from_server_config() {
        let mut config = HashMap::new();
        config.insert("http.port".to_string(), "8090".to_string());
        config.insert("rtsp.port".to_string(), "8554".to_string());
        config.insert("rtmp.port".to_string(), "1935".to_string());

        let port_config = ZlmPortConfig::from_server_config(&config);
        assert_eq!(port_config.http_port, 8090);
        assert_eq!(port_config.rtsp_port, 8554);
    }
}
