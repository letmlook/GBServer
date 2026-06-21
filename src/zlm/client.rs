use reqwest::Client;
use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::collections::HashMap;

use super::types::*;
use crate::config::ZlmServerConfig;

#[derive(Clone)]
pub struct ZlmClient {
    base_url: String,
    pub secret: String,
    pub ip: String,
    pub http_port: u16,
    http: Client,
}

/// ZLM `/index/api/openRtpServer` 的真实响应：成功时 `port` / `cookie` 在
/// 顶层而非 `data` 字段里（ZLM 简单 API 的惯例，区别于 listRtpServer
/// 那种 `data: [...]` 形式）。用扁平结构接收，避免 `data: Some(RtpServerInfo)`
/// 因 `RtpServerInfo.stream_id` 必填而整个反序列化失败、导致调用方
/// 看到 "No RTP server info returned"。
#[derive(Deserialize)]
struct OpenRtpServerResp {
    code: i32,
    #[serde(default)]
    msg: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    cookie: Option<String>,
}

impl ZlmClient {
    pub fn new(ip: &str, port: u16, secret: &str) -> Self {
        Self {
            base_url: format!("http://{}:{}", ip, port),
            secret: secret.to_string(),
            ip: ip.to_string(),
            http_port: port,
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    pub fn from_config(config: &ZlmServerConfig) -> Self {
        Self::new(&config.ip, config.http_port, &config.secret)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn request<R: for<'de> serde::Deserialize<'de>>(&self, path: &str, params: &[(&str, String)]) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.http.get(&url);
        for (k, v) in params {
            req = req.query(&[(k, v)]);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("HTTP error: {}", resp.status()));
        }
        let body: R = resp.json().await?;
        Ok(body)
    }

    async fn request_post<R: for<'de> serde::Deserialize<'de>, B: serde::Serialize>(&self, path: &str, body: &B) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http.post(&url).json(body).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("HTTP error: {}", resp.status()));
        }
        let body: R = resp.json().await?;
        Ok(body)
    }

    pub async fn get_media_list(&self, schema: Option<&str>, app: Option<&str>, stream: Option<&str>) -> Result<Vec<MediaInfo>> {
        let mut params = vec![("secret", self.secret.clone())];
        if let Some(s) = schema { params.push(("schema", s.to_string())); }
        if let Some(a) = app { params.push(("app", a.to_string())); }
        if let Some(s) = stream { params.push(("stream", s.to_string())); }

        let resp: ApiResponse<Vec<MediaInfo>> = self.request("/index/api/getMediaList", &params).await?;
        Ok(resp.data.unwrap_or_default())
    }

    /// 获取当前活跃的流数量（用于负载均衡）
    pub async fn get_active_stream_count(&self) -> Result<usize> {
        let list = self.get_media_list(None, None, None).await?;
        Ok(list.len())
    }

    pub async fn get_media_info(&self, schema: &str, vhost: &str, app: &str, stream: &str) -> Result<Option<MediaInfo>> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("schema", schema.to_string()),
            ("vhost", vhost.to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
        ];

        let resp: ApiResponse<MediaInfo> = self.request("/index/api/getMediaInfo", &params).await?;
        Ok(resp.data.filter(|d| d.schema == schema && d.app == app && d.stream == stream))
    }

    pub async fn is_media_exist(&self, schema: &str, vhost: &str, app: &str, stream: &str) -> Result<bool> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("schema", schema.to_string()),
            ("vhost", vhost.to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
        ];

        #[derive(Deserialize)]
        struct ExistResp { exist: bool }
        let resp: ApiResponse<ExistResp> = self.request("/index/api/isMediaExist", &params).await?;
        Ok(resp.data.map(|r| r.exist).unwrap_or(false))
    }

    pub async fn add_stream_proxy(&self, req: &AddStreamProxyRequest) -> Result<String> {
        let mut params = vec![
            ("secret", self.secret.clone()),
            ("vhost", req.vhost.clone()),
            ("app", req.app.clone()),
            ("stream", req.stream.clone()),
            ("url", req.url.clone()),
        ];
        if let Some(rtp) = req.rtp_type { params.push(("rtp_type", rtp.to_string())); }
        if let Some(t) = req.timeout_sec { params.push(("timeout_sec", t.to_string())); }
        if let Some(v) = req.enable_hls { params.push(("enable_hls", if v { "1" } else { "0" }.to_string())); }
        if let Some(v) = req.enable_mp4 { params.push(("enable_mp4", if v { "1" } else { "0" }.to_string())); }
        if let Some(v) = req.enable_rtsp { params.push(("enable_rtsp", if v { "1" } else { "0" }.to_string())); }
        if let Some(v) = req.enable_rtmp { params.push(("enable_rtmp", if v { "1" } else { "0" }.to_string())); }
        if let Some(v) = req.enable_fmp4 { params.push(("enable_fmp4", if v { "1" } else { "0" }.to_string())); }
        if let Some(v) = req.enable_ts { params.push(("enable_ts", if v { "1" } else { "0" }.to_string())); }
        if let Some(v) = req.enableAAC { params.push(("enable_aac", if v { "1" } else { "0" }.to_string())); }

        #[derive(Deserialize)]
        struct Resp { key: String }
        let resp: ApiResponse<Resp> = self.request("/index/api/addStreamProxy", &params).await?;
        
        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {} - {}", resp.code, resp.msg.unwrap_or_default()));
        }
        Ok(resp.data.map(|r| r.key).unwrap_or_default())
    }

    pub async fn add_stream_proxy_async(&self, req: &AddStreamProxyRequest) -> Result<String> {
        self.add_stream_proxy(req).await
    }

    pub async fn close_streams(&self, schema: Option<&str>, app: Option<&str>, stream: Option<&str>, force: bool) -> Result<CloseStreamsResponse> {
        let mut params = vec![
            ("secret", self.secret.clone()), 
            ("force", if force { "1" } else { "0" }.to_string())
        ];
        if let Some(s) = schema { params.push(("schema", s.to_string())); }
        if let Some(a) = app { params.push(("app", a.to_string())); }
        if let Some(s) = stream { params.push(("stream", s.to_string())); }

        let resp: ApiResponse<CloseStreamsResponse> = self.request("/index/api/close_streams", &params).await?;
        Ok(resp.data.unwrap_or(CloseStreamsResponse { count_hit: 0, count_closed: 0 }))
    }

    pub async fn close_stream(&self, vhost: &str, app: &str, stream: &str, force: bool) -> Result<()> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("vhost", vhost.to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
            ("force", if force { "1" } else { "0" }.to_string()),
        ];

        #[derive(Deserialize)]
        struct Resp { code: i32 }
        let resp: ApiResponse<Resp> = self.request("/index/api/close_stream", &params).await?;
        
        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }

    pub async fn kick_session(&self, vhost: &str, app: &str, stream: &str, schema: &str) -> Result<u32> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("vhost", vhost.to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
            ("schema", schema.to_string()),
        ];

        let resp: ApiResponse<KickSessionResponse> = self.request("/index/api/kick_session", &params).await?;
        Ok(resp.data.map(|r| r.hit).unwrap_or(0))
    }

    pub async fn kick_all_sessions(&self, vhost: &str, app: &str, stream: &str) -> Result<u32> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("vhost", vhost.to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
        ];

        #[derive(Deserialize)]
        struct Resp { count: u32 }
        let resp: ApiResponse<Resp> = self.request("/index/api/kick_sessions", &params).await?;
        Ok(resp.data.map(|r| r.count).unwrap_or(0))
    }

    pub async fn open_rtp_server(&self, req: &OpenRtpServerRequest) -> Result<RtpServerInfo> {
        let mut params = vec![
            ("secret", self.secret.clone()),
            ("stream_id", req.stream_id.clone()),
        ];

        if let Some(port) = req.port {
            params.push(("port", port.to_string()));
        }
        if let Some(use_tcp) = req.use_tcp {
            params.push(("tcp", if use_tcp { "1" } else { "0" }.to_string()));
        }
        if let Some(rtp_type) = req.rtp_type {
            params.push(("rtp_type", rtp_type.to_string()));
        }
        if let Some(recv_port) = req.recv_port {
            params.push(("recv_port", recv_port.to_string()));
        }

        let resp: OpenRtpServerResp = self.request("/index/api/openRtpServer", &params).await?;

        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {} - {}", resp.code, resp.msg.unwrap_or_default()));
        }

        let port = resp.port.ok_or_else(|| anyhow!("No RTP server info returned"))?;

        // ZLM openRtpServer 不回传 stream_id / ssrc 等字段，从入参回填；
        // 调用方（playback / play / broadcast / cascade）已经持有 stream_id，
        // 这样下游继续用 `RtpServerInfo.stream_id` 不会变 None。
        Ok(RtpServerInfo {
            port,
            stream_id: req.stream_id.clone(),
            ssrc: None,
            client_ip: None,
            client_port: None,
            server_port: None,
            selectrtp_conn: None,
        })
    }

    pub async fn close_rtp_server(&self, stream_id: &str) -> Result<()> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("stream_id", stream_id.to_string()),
        ];

        #[derive(Deserialize)]
        struct Resp { code: i32 }
        let resp: ApiResponse<Resp> = self.request("/index/api/closeRtpServer", &params).await?;

        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }

    /// 让 ZLM 主动连接远端的 RTP 服务器(`设备` 侧 GB28181 INVITE 200 OK
    /// SDP 里的 m= 端口 —— 设备宣告自己将从该端口发送流)。
    /// 解决了"设备按 200 OK SDP 推流而不是按 INVITE m= 推流"的非标准
    /// gbcpp/1.0 mock 行为;WVP2.6.9 同样依赖此机制。
    pub async fn connect_rtp_server(
        &self,
        stream_id: &str,
        dst_url: &str, // 例如 "rtp://192.168.3.200:11001"
        app: Option<&str>,
    ) -> Result<()> {
        let mut params = vec![
            ("secret", self.secret.clone()),
            ("stream_id", stream_id.to_string()),
            ("dst_url", dst_url.to_string()),
        ];
        if let Some(a) = app {
            params.push(("app", a.to_string()));
        }

        #[derive(Deserialize)]
        struct Resp { code: i32 }
        let resp: ApiResponse<Resp> = self.request("/index/api/connectRtpServer", &params).await?;

        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }

    pub async fn get_rtp_info(&self, stream_id: &str) -> Result<Option<RtpInfo>> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("stream_id", stream_id.to_string()),
        ];

        let resp: ApiResponse<RtpInfo> = self.request("/index/api/getRtpInfo", &params).await?;
        Ok(resp.data)
    }

    pub async fn list_rtp_servers(&self) -> Result<Vec<RtpServerInfo>> {
        let params = vec![("secret", self.secret.clone())];
        
        let resp: ApiResponse<Vec<RtpServerInfo>> = self.request("/index/api/listRtpServer", &params).await?;
        Ok(resp.data.unwrap_or_default())
    }

    pub async fn start_record(&self, type_: &str, vhost: &str, app: &str, stream: &str) -> Result<()> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("type", type_.to_string()),
            ("vhost", vhost.to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
        ];
        
        let resp: ApiResponse<serde_json::Value> = self.request("/index/api/startRecord", &params).await?;
        
        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }

    pub async fn stop_record(&self, type_: &str, vhost: &str, app: &str, stream: &str) -> Result<()> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("type", type_.to_string()),
            ("vhost", vhost.to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
        ];
        
        let resp: ApiResponse<serde_json::Value> = self.request("/index/api/stopRecord", &params).await?;
        
        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }

    pub async fn is_recording(&self, vhost: &str, app: &str, stream: &str) -> Result<bool> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("vhost", vhost.to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
        ];
        
        #[derive(Deserialize)]
        struct Resp { exist: bool }
        let resp: ApiResponse<Resp> = self.request("/index/api/isRecording", &params).await?;
        Ok(resp.data.map(|r| r.exist).unwrap_or(false))
    }

    pub async fn get_mp4_record_file(&self, app: &str, stream: &str, path: Option<&str>, start_time: Option<&str>, end_time: Option<&str>) -> Result<Vec<Mp4RecordFile>> {
        let mut params = vec![
            ("secret", self.secret.clone()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
        ];
        if let Some(p) = path { params.push(("path", p.to_string())); }
        if let Some(s) = start_time { params.push(("start_time", s.to_string())); }
        if let Some(e) = end_time { params.push(("end_time", e.to_string())); }

        let resp: ApiResponse<Mp4RecordResponse> = self.request("/index/api/getMp4RecordFile", &params).await?;
        Ok(resp.data.map(|r| r.list).unwrap_or_default())
    }

    pub async fn delete_mp4_file(&self, file_path: &str) -> Result<()> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("file_path", file_path.to_string()),
        ];

        #[derive(Deserialize)]
        struct Resp { code: i32 }
        let resp: ApiResponse<Resp> = self.request("/index/api/deleteRecord", &params).await?;
        
        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }

    pub async fn get_snap(&self, url: &str, timeout_sec: Option<f64>, save_path: Option<&str>) -> Result<String> {
        let mut params = vec![
            ("secret", self.secret.clone()),
            ("url", url.to_string()),
        ];
        if let Some(t) = timeout_sec { params.push(("timeout_sec", t.to_string())); }
        if let Some(p) = save_path { params.push(("save_path", p.to_string())); }

        let resp: ApiResponse<SnapResponse> = self.request("/index/api/getSnap", &params).await?;
        
        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(resp.data.and_then(|r| r.path).unwrap_or_default())
    }

    pub async fn add_ffmpeg_source(&self, req: &AddFFmpegSourceRequest) -> Result<String> {
        let mut params = vec![
            ("secret", self.secret.clone()),
            ("src_url", req.src_url.clone()),
            ("dst_url", req.dst_url.clone()),
        ];
        
        if let Some(t) = req.timeout_ms { params.push(("timeout_ms", t.to_string())); }
        if let Some(k) = &req.ffmpeg_cmd_key { params.push(("ffmpeg_cmd_key", k.clone())); }
        if let Some(v) = req.enable_hls { params.push(("enable_hls", if v { "1" } else { "0" }.to_string())); }
        if let Some(v) = req.enable_mp4 { params.push(("enable_mp4", if v { "1" } else { "0" }.to_string())); }
        if let Some(v) = req.enable_rtsp { params.push(("enable_rtsp", if v { "1" } else { "0" }.to_string())); }
        if let Some(v) = req.enable_rtmp { params.push(("enable_rtmp", if v { "1" } else { "0" }.to_string())); }
        if let Some(v) = req.enable_fmp4 { params.push(("enable_fmp4", if v { "1" } else { "0" }.to_string())); }

        #[derive(Deserialize)]
        struct Resp { key: String }
        let resp: ApiResponse<Resp> = self.request("/index/api/addFfmpegSource", &params).await?;
        
        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(resp.data.map(|r| r.key).unwrap_or_default())
    }

    pub async fn get_server_config(&self) -> Result<HashMap<String, String>> {
        let params = vec![("secret", self.secret.clone())];
        
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "api.apiDebug")]
            api_debug: Option<String>,
            #[serde(flatten)]
            rest: HashMap<String, String>,
        }
        
        let resp: ApiResponse<Vec<Resp>> = self.request("/index/api/getServerConfig", &params).await?;
        let mut result = HashMap::new();
        if let Some(data) = resp.data {
            for item in data {
                result.extend(item.rest);
            }
        }
        Ok(result)
    }

    /// Set a single ZLM server config key-value pair
    pub async fn set_server_config(&self, secret: &str, key: &str, value: &str) -> Result<()> {
        #[derive(serde::Serialize)]
        struct SetConfigReq {
            secret: String,
            #[serde(rename = "key")]
            key_: String,
            value: String,
        }
        let req = SetConfigReq {
            secret: secret.to_string(),
            key_: key.to_string(),
            value: value.to_string(),
        };
        #[derive(Deserialize)]
        struct Resp {
            code: i32,
        }
        let resp: Resp = self.request_post("/index/api/setServerConfig", &req).await?;
        if resp.code == 0 {
            Ok(())
        } else {
            Err(anyhow!("setServerConfig failed with code {}", resp.code))
        }
    }

    pub async fn get_server_stats(&self) -> Result<HashMap<String, serde_json::Value>> {
        let params = vec![("secret", self.secret.clone())];
        let resp: ApiResponse<HashMap<String, serde_json::Value>> = self.request("/index/api/getServerStats", &params).await?;
        Ok(resp.data.unwrap_or_default())
    }

    pub async fn get_net_work_api(&self) -> Result<HashMap<String, serde_json::Value>> {
        let params = vec![("secret", self.secret.clone())];
        let resp: ApiResponse<HashMap<String, serde_json::Value>> = self.request("/index/api/getNetWorkApi", &params).await?;
        Ok(resp.data.unwrap_or_default())
    }

    pub async fn get_media_video_info(&self, url: &str) -> Result<serde_json::Value> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("url", url.to_string()),
        ];
        let resp: ApiResponse<serde_json::Value> = self.request("/index/api/getMediaInfo", &params).await?;
        resp.data.ok_or_else(|| anyhow!("No media info"))
    }

    pub async fn restart_server(&self) -> Result<()> {
        let params = vec![("secret", self.secret.clone())];
        
        #[derive(Deserialize)]
        struct Resp { code: i32 }
        let resp: ApiResponse<Resp> = self.request("/index/api/restartServer", &params).await?;
        
        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }

    pub async fn send_rtp_info(&self, stream_id: &str, ssrc: &str, client_ip: &str, client_port: u16) -> Result<()> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("stream_id", stream_id.to_string()),
            ("ssrc", ssrc.to_string()),
            ("client_ip", client_ip.to_string()),
            ("client_port", client_port.to_string()),
        ];

        #[derive(Deserialize)]
        struct Resp { code: i32 }
        let resp: ApiResponse<Resp> = self.request("/index/api/sendRtpInfo", &params).await?;

        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }

    /// B3: 启动 ZLM 将本地 RTP 流推送到上级平台 (startSendRtp)
    ///
    /// 由 SipServer 在收到设备 INVITE 200 OK 后调用，把本级已接收的设备 RTP
    /// 通过 ZLM 转发到上级平台指定的 IP:port。
    pub async fn start_send_rtp(
        &self,
        vhost: &str,
        app: &str,
        stream: &str,
        ssrc: &str,
        dst_url: &str,
        dst_port: u16,
        is_udp: bool,
        src_port: Option<u16>,
        use_ps: bool,
    ) -> Result<()> {
        let mut params = vec![
            ("secret", self.secret.clone()),
            ("vhost", vhost.to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
            ("ssrc", ssrc.to_string()),
            ("dst_url", dst_url.to_string()),
            ("dst_port", dst_port.to_string()),
            ("is_udp", (if is_udp { 1 } else { 0 }).to_string()),
            ("use_ps", (if use_ps { 1 } else { 0 }).to_string()),
        ];
        if let Some(p) = src_port {
            params.push(("src_port", p.to_string()));
        }

        #[derive(Deserialize)]
        struct Resp { code: i32 }
        let resp: ApiResponse<Resp> = self.request("/index/api/startSendRtp", &params).await?;

        if resp.code != 0 {
            return Err(anyhow!("ZLM startSendRtp error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }

    /// B3: 停止 ZLM 向某个上级平台的 SendRtp 推送 (stopSendRtp)
    pub async fn stop_send_rtp(
        &self,
        vhost: &str,
        app: &str,
        stream: &str,
    ) -> Result<()> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("vhost", vhost.to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
        ];

        #[derive(Deserialize)]
        struct Resp { code: i32 }
        let resp: ApiResponse<Resp> = self.request("/index/api/stopSendRtp", &params).await?;

        if resp.code != 0 {
            return Err(anyhow!("ZLM stopSendRtp error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }

    pub async fn create_download(&self, url: &str, file_name: &str, save_path: Option<&str>) -> Result<String> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("url", url.to_string()),
            ("file_name", file_name.to_string()),
            ("save_path", save_path.unwrap_or("./").to_string()),
        ];

        #[derive(Deserialize)]
        struct Resp { path: String }
        let resp: ApiResponse<Resp> = self.request("/index/api/createDownload", &params).await?;
        
        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(resp.data.map(|r| r.path).unwrap_or_default())
    }

    pub async fn get_download_list(&self) -> Result<Vec<DownloadInfo>> {
        let params = vec![("secret", self.secret.clone())];
        
        #[derive(Deserialize)]
        struct Resp {
            list: Vec<DownloadInfo>
        }
        
        #[derive(Deserialize)]
        struct InnerResp {
            data: Option<Resp>
        }
        
        let resp: ApiResponse<Resp> = self.request("/index/api/getDownloadList", &params).await?;
        Ok(resp.data.map(|r| r.list).unwrap_or_default())
    }

    pub async fn stop_download(&self, file_name: &str) -> Result<()> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("file_name", file_name.to_string()),
        ];

        #[derive(Deserialize)]
        struct Resp { code: i32 }
        let resp: ApiResponse<Resp> = self.request("/index/api/close_download", &params).await?;
        
        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }
}

impl std::fmt::Debug for ZlmClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZlmClient")
            .field("base_url", &self.base_url)
            .field("secret", &"[hidden]")
            .finish()
    }
}

// ============================================================================
// Phase 4.3: parse_port_range — "start,end" → (u16, u16)
// ============================================================================

/// Parse a port range string in the form `"start,end"` (comma-separated).
///
/// Returns the start and end ports as `(u16, u16)` on success.
/// Returns an error if:
/// - the input does not contain exactly one comma separator (i.e. not 2 parts)
/// - either part fails to parse as a `u16`
///
/// The output format expected by ZLM's `setServerConfig("rtp.port_range", ...)`
/// is `"start-end"` (dash-separated), so callers typically do:
/// `format!("{}-{}", start, end)`.
pub fn parse_port_range(s: &str) -> Result<(u16, u16)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid port range: {} (expected 'start,end')", s));
    }
    let start: u16 = parts[0].parse().map_err(|e| {
        anyhow!("Invalid port range start '{}': {}", parts[0], e)
    })?;
    let end: u16 = parts[1].parse().map_err(|e| {
        anyhow!("Invalid port range end '{}': {}", parts[1], e)
    })?;
    Ok((start, end))
}

/// Set a ZLM RTP port range config key from a raw `"start,end"` string.
///
/// This is a convenience wrapper that calls `parse_port_range`, formats the
/// result as `"start-end"`, and then calls `zlm.set_server_config(secret, key, &value)`.
///
/// Returns `Ok(())` on success; propagates errors from `parse_port_range` or
/// `set_server_config`.
pub async fn set_rtp_port_range(
    zlm: &ZlmClient,
    secret: &str,
    key: &str,
    raw: &str,
) -> Result<()> {
    let (start, end) = parse_port_range(raw)?;
    let value = format!("{}-{}", start, end);
    zlm.set_server_config(secret, key, &value).await
}

// ============================================================================
// Phase 4.2: ZLM 媒体节点健康状态扩展
// ============================================================================

/// ZLM 节点健康状态（新增到 ZlmClient）
#[derive(Debug, Clone)]
pub struct ZlmHealthState {
    /// 节点是否在线
    pub online: bool,
    /// 最后心跳时间（秒时间戳）
    pub last_keepalive: i64,
    /// 当前活跃流数量
    pub stream_count: i32,
    /// 当前 RTP Server 数量
    pub rtp_server_count: i32,
    /// 最后查询错误
    pub last_error: Option<String>,
}

impl ZlmClient {
    /// 探测节点是否可达（轻量级 ZLM API 检查）
    pub async fn is_alive(&self) -> bool {
        match self.get_api_version().await {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// 获取当前活跃流数量（从 ZLM API 获取）
    pub async fn get_stream_count(&self) -> Result<i32, reqwest::Error> {
        #[derive(Deserialize)]
        struct StreamListResp {
            #[serde(rename = "code")]
            code: i32,
            #[serde(rename = "data")]
            data: Option<Vec<serde_json::Value>>,
        }
        let resp: StreamListResp = self.http
            .get(format!("{}/api/stream/list", self.base_url))
            .query(&[("secret", &self.secret)])
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.data.map(|v| v.len() as i32).unwrap_or(0))
    }

    /// 获取 API 版本（用于存活探测）
    pub async fn get_api_version(&self) -> Result<String, reqwest::Error> {
        #[derive(Deserialize)]
        struct VersionResp {
            #[serde(rename = "code")]
            code: i32,
            #[serde(rename = "version")]
            version: Option<String>,
        }
        let resp: VersionResp = self.http
            .get(format!("{}/api/version", self.base_url))
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.version.unwrap_or_else(|| "unknown".to_string()))
    }

    /// 获取节点健康状态快照
    pub async fn health_snapshot(&self) -> ZlmHealthState {
        ZlmHealthState {
            online: self.is_alive().await,
            last_keepalive: chrono::Utc::now().timestamp(),
            stream_count: self.get_stream_count().await.unwrap_or(-1),
            rtp_server_count: 0, // 可通过 get_rtp_server_list API 获取
            last_error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_port_range_valid() {
        let (start, end) = parse_port_range("30000,30200").expect("valid range should parse");
        assert_eq!(start, 30000);
        assert_eq!(end, 30200);

        // 边界值：u16 min/max
        let (s2, e2) = parse_port_range("0,65535").expect("boundary range should parse");
        assert_eq!(s2, 0);
        assert_eq!(e2, 65535);
    }

    #[test]
    fn test_parse_port_range_invalid_format() {
        // 只有一段（缺逗号）
        assert!(parse_port_range("30000").is_err());
        // 多于两段（多逗号）
        assert!(parse_port_range("30000,30100,30200").is_err());
        // 空字符串
        assert!(parse_port_range("").is_err());
        // 仅逗号
        assert!(parse_port_range(",").is_err());
    }

    #[test]
    fn test_parse_port_range_non_numeric() {
        // 起始非数字
        assert!(parse_port_range("abc,30100").is_err());
        // 结束非数字
        assert!(parse_port_range("30000,xyz").is_err());
        // 超出 u16
        assert!(parse_port_range("30000,99999").is_err());
        // 负数（u16 parse 失败）
        assert!(parse_port_range("-1,30100").is_err());
    }

    // Phase 4.3: wiremock test for set_rtp_port_range
    mod wiremock {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        #[tokio::test]
        async fn test_set_rtp_port_range_calls_set_server_config() {
            let mock_server = MockServer::start().await;

            Mock::given(method("POST"))
                .and(path("/index/api/setServerConfig"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "code": 0
                })))
                .expect(1)
                .mount(&mock_server)
                .await;

            let uri = mock_server.uri();
            let stripped = uri.trim_start_matches("http://");
            let mut parts = stripped.splitn(2, ':');
            let ip = parts.next().unwrap_or("127.0.0.1").to_string();
            let port: u16 = parts
                .next()
                .and_then(|p| p.parse().ok())
                .unwrap_or(80);

            let zlm_client = crate::zlm::ZlmClient::new(&ip, port, "test-secret");

            // "30000,30200" → "30000-30200"
            super::set_rtp_port_range(&zlm_client, "test-secret", "rtp.port_range", "30000,30200")
                .await
                .expect("set_rtp_port_range should succeed");

            let received = mock_server.received_requests().await.unwrap_or_default();
            assert_eq!(received.len(), 1, "expected exactly 1 request");
            let body = String::from_utf8_lossy(&received[0].body).to_string();
            assert!(
                body.contains("\"key\":\"rtp.port_range\"") && body.contains("30000-30200"),
                "expected key='rtp.port_range' and value='30000-30200', got: {}",
                body
            );
        }
    }
}
