use reqwest::Client;
use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::collections::HashMap;

use super::types::*;
use crate::config::{ZlmServerConfig, ZlmConfig};

#[derive(Clone)]
pub struct ZlmClient {
    base_url: String,
    pub secret: String,
    http: Client,
}

impl ZlmClient {
    pub fn new(ip: &str, port: u16, secret: &str) -> Self {
        Self {
            base_url: format!("http://{}:{}", ip, port),
            secret: secret.to_string(),
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    pub fn from_config(config: &ZlmServerConfig) -> Self {
        Self::new(&config.ip, config.http_port, &config.secret)
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

        let resp: ApiResponse<RtpServerInfo> = self.request("/index/api/openRtpServer", &params).await?;
        
        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {} - {}", resp.code, resp.msg.unwrap_or_default()));
        }
        
        resp.data.ok_or_else(|| anyhow!("No RTP server info returned"))
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
