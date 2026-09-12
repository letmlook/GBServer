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

/// ZLM 的 **HTTP-FLV** 地址。
///
/// 后缀**固定是 `.live.flv`**（不是 `.flv`）。实测 zlmediakit/zlmediakit:master
/// （git fdaec26）：
/// * `GET /rtp/<stream>.live.flv` → `FLV\x01…`，真正的流；
/// * `GET /rtp/<stream>.flv`      → 404 的 HTML 页面。
///
/// 此前 `play_urls_json` / `common_channel::channel_play` / `playback` /
/// `device_query` 等多处拼的是 `.flv` —— 浏览器里必然 404，而实时预览页的
/// 兜底顺序是 `hls → flvUrl → playUrl`：HLS 对 RTP 流本来就没有源，
/// 于是**两个都在 404，GB28181 通道在浏览器里彻底播不了**。
pub fn http_flv_url(ip: &str, http_port: u16, app: &str, stream: &str) -> String {
    format!("http://{ip}:{http_port}/{app}/{stream}.live.flv")
}

/// ZLM 的 **WebSocket-FLV** 地址（同样以 `.live.flv` 结尾）。
pub fn ws_flv_url(ip: &str, http_port: u16, app: &str, stream: &str) -> String {
    format!("ws://{ip}:{http_port}/{app}/{stream}.live.flv")
}

/// ZLM 的 **HLS** 地址。
///
/// 注意：ZLM 并非对每一路流都生成 HLS（GB28181 的 RTP/PS 流在本仓库实测的镜像上
/// 就没有 `hls` 源）。调用方应先用 `ZlmClient::has_schema(.., "hls")` 探测，
/// **只在真的有源时**才把它放进响应里。
pub fn hls_url(ip: &str, http_port: u16, app: &str, stream: &str) -> String {
    format!("http://{ip}:{http_port}/{app}/{stream}/hls.m3u8")
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
            flv: http_flv_url(
                &self.ip,
                self.port_config.flv_port.unwrap_or(self.port_config.http_port),
                app,
                stream_id,
            ),
            ws_flv: ws_flv_url(
                &self.ip,
                self.port_config.ws_flv_port.unwrap_or(self.port_config.http_port),
                app,
                stream_id,
            ),
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

    /// HTTP-FLV / WS-FLV 的后缀必须是 `.live.flv`（实测 `.flv` 在 ZLM 上是 404 HTML）。
    #[test]
    fn flv_urls_use_live_suffix() {
        assert_eq!(
            http_flv_url("10.0.0.1", 8080, "rtp", "s1"),
            "http://10.0.0.1:8080/rtp/s1.live.flv"
        );
        assert_eq!(
            ws_flv_url("10.0.0.1", 8080, "rtp", "s1"),
            "ws://10.0.0.1:8080/rtp/s1.live.flv"
        );
        assert_eq!(
            hls_url("10.0.0.1", 8080, "rtp", "s1"),
            "http://10.0.0.1:8080/rtp/s1/hls.m3u8"
        );
        let addrs = StreamAddressBuilder::new("10.0.0.1", ZlmPortConfig::default()).build("rtp", "s1");
        assert!(addrs.flv.ends_with(".live.flv"), "{}", addrs.flv);
        assert!(addrs.ws_flv.ends_with(".live.flv"), "{}", addrs.ws_flv);
    }

    #[test]
    fn test_build_addresses() {
        let config = ZlmPortConfig::default();
        let builder = StreamAddressBuilder::new("192.168.1.100", config);
        let addrs = builder.build("rtp", "device1_channel1");

        assert!(addrs.rtsp.starts_with("rtsp://192.168.1.100:554/"));
        assert!(addrs.rtmp.starts_with("rtmp://192.168.1.100:1935/"));
        assert!(addrs.hls.contains("hls.m3u8"));
        assert!(addrs.flv.ends_with(".live.flv"), "HTTP-FLV 后缀固定 .live.flv: {}", addrs.flv);
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
