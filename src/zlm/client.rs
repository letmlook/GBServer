//! ZLM HTTP API 客户端实现

use reqwest::Client;
use anyhow::Result;
use serde::Deserialize;

use super::types::*;
use crate::config::{ZlmServerConfig, ZlmConfig};

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
            http: Client::new(),
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
        let body: R = resp.json().await?;
        Ok(body)
    }

    async fn request_post<R: for<'de> serde::Deserialize<'de>, B: serde::Serialize>(&self, path: &str, body: &B) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http.post(&url).json(body).send().await?;
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

        #[derive(Deserialize)]
        struct Resp { key: String }
        let resp: ApiResponse<Resp> = self.request("/index/api/addStreamProxy", &params).await?;
        Ok(resp.data.map(|r| r.key).unwrap_or_default())
    }

    pub async fn close_streams(&self, schema: Option<&str>, app: Option<&str>, stream: Option<&str>, force: bool) -> Result<CloseStreamsResponse> {
        let mut params = vec![("secret", self.secret.clone()), ("force", if force { "1" } else { "0" }.to_string())];
        if let Some(s) = schema { params.push(("schema", s.to_string())); }
        if let Some(a) = app { params.push(("app", a.to_string())); }
        if let Some(s) = stream { params.push(("stream", s.to_string())); }

        let resp: ApiResponse<CloseStreamsResponse> = self.request("/index/api/close_streams", &params).await?;
        Ok(resp.data.unwrap_or(CloseStreamsResponse { count_hit: 0, count_closed: 0 }))
    }

    pub async fn start_record(&self, type_: &str, vhost: &str, app: &str, stream: &str) -> Result<()> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("type", type_.to_string()),
            ("vhost", vhost.to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
        ];
        #[derive(Deserialize)]
        struct Resp { code: i32 }
        let _: ApiResponse<Resp> = self.request("/index/api/startRecord", &params).await?;
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
        #[derive(Deserialize)]
        struct Resp { code: i32 }
        let _: ApiResponse<Resp> = self.request("/index/api/stopRecord", &params).await?;
        Ok(())
    }

    pub async fn get_mp4_record_file(&self, app: &str, stream: &str, path: Option<&str>) -> Result<Vec<Mp4RecordFile>> {
        let mut params = vec![
            ("secret", self.secret.clone()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
        ];
        if let Some(p) = path { params.push(("path", p.to_string())); }

        let resp: ApiResponse<Mp4RecordResponse> = self.request("/index/api/getMp4RecordFile", &params).await?;
        Ok(resp.data.map(|r| r.list).unwrap_or_default())
    }

    pub async fn get_snap(&self, url: &str, timeout_sec: Option<f64>, save_path: Option<&str>) -> Result<String> {
        let mut params = vec![
            ("secret", self.secret.clone()),
            ("url", url.to_string()),
        ];
        if let Some(t) = timeout_sec { params.push(("timeout_sec", t.to_string())); }
        if let Some(p) = save_path { params.push(("save_path", p.to_string())); }

        let resp: ApiResponse<SnapResponse> = self.request("/index/api/getSnap", &params).await?;
        Ok(resp.data.and_then(|r| r.path).unwrap_or_default())
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

    pub async fn get_server_config(&self) -> Result<std::collections::HashMap<String, String>> {
        let params = vec![("secret", self.secret.clone())];
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "api.apiDebug")]
            api_debug: Option<String>,
            #[serde(flatten)]
            rest: std::collections::HashMap<String, String>,
        }
        let resp: ApiResponse<Vec<Resp>> = self.request("/index/api/getServerConfig", &params).await?;
        let mut result = std::collections::HashMap::new();
        if let Some(data) = resp.data {
            for item in data {
                result.extend(item.rest);
            }
        }
        Ok(result)
    }
}
