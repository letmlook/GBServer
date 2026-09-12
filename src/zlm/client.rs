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
/// 从 `isRecording` / `isMediaExist` 的响应里取布尔结论。
///
/// ZLMediaKit 的"简单 API"用 `throw ApiRet("exist", …)` 返回**扁平**结构：
/// `{"code":0,"exist":true}` —— 字段在顶层，不在 `data` 里。此前这两个方法
/// 都用 `ApiResponse<Resp>`（要求 `data.exist`）解析，于是**在真实 ZLM 上
/// 恒为 false**，只有在返回 `data:{exist:...}` 的 mock 上才碰巧正确：
///
/// * `is_media_exist` 恒 false ⇒ "先起流再取地址"等判断全部走错分支；
/// * `is_recording` 恒 false ⇒ 云录像是否在录的判定失真。
///
/// 这里兼容三种已知形态（顶层 `exist` / `data.exist` / 旧 mock 的 `status`）。
/// 解析 `getRtpInfo` 的响应，兼容扁平与嵌套两种形态。
fn parse_rtp_info(stream_id: &str, raw: &serde_json::Value) -> Option<RtpInfo> {
    let get_str = |k: &str| raw.get(k).and_then(|v| v.as_str()).map(str::to_string);
    let get_u16 = |k: &str| raw.get(k).and_then(|v| v.as_u64()).map(|v| v as u16);
    let get_u32 = |k: &str| raw.get(k).and_then(|v| v.as_u64()).map(|v| v as u32);

    // 扁平形态（真实 ZLM master）
    if let Some(exist) = raw.get("exist").and_then(|v| v.as_bool()) {
        if !exist {
            return None;
        }
        return Some(RtpInfo {
            stream_id: get_str("identifier").unwrap_or_else(|| stream_id.to_string()),
            ssrc: get_str("ssrc")
                .or_else(|| raw.get("ssrc").map(|v| v.to_string()))
                .unwrap_or_default(),
            peer_ip: get_str("peer_ip").unwrap_or_default(),
            peer_port: get_u16("peer_port").unwrap_or(0),
            local_port: get_u16("local_port").unwrap_or(0),
            alive_second: get_u32("alive_second").unwrap_or(0),
            rtt: get_u32("rtt").unwrap_or(0),
        });
    }

    // 嵌套形态（部分版本/旧 mock）：{"code":0,"data":{...}}
    let data = raw.get("data").filter(|d| !d.is_null())?;
    serde_json::from_value::<RtpInfo>(data.clone()).ok()
}

fn exist_flag(resp: &serde_json::Value) -> bool {
    let at = |v: &serde_json::Value, key: &str| v.get(key).and_then(|x| x.as_bool());
    if let Some(b) = at(resp, "exist").or_else(|| at(resp, "status")) {
        return b;
    }
    if let Some(data) = resp.get("data") {
        if let Some(b) = at(data, "exist").or_else(|| at(data, "status")) {
            return b;
        }
    }
    false
}

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
    // ZLM 响应字段：仅用于反序列化，代码不读取（保留以完整映射 API）
    #[serde(default)]
    #[allow(dead_code)]
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

        let resp: serde_json::Value = self.request("/index/api/isMediaExist", &params).await?;
        Ok(exist_flag(&resp))
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
        #[allow(dead_code)]
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
        #[allow(dead_code)]
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
        dst_port: u16, // ZLM 要求 dst_port 单独传(不是合并在 URL 里)
        app: Option<&str>,
    ) -> Result<()> {
        let mut params = vec![
            ("secret", self.secret.clone()),
            ("stream_id", stream_id.to_string()),
            ("dst_url", dst_url.to_string()),
            ("dst_port", dst_port.to_string()),
        ];
        if let Some(a) = app {
            params.push(("app", a.to_string()));
        }

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct Resp { code: i32 }
        let resp: ApiResponse<Resp> = self.request("/index/api/connectRtpServer", &params).await?;

        if resp.code != 0 {
            return Err(anyhow!("ZLM error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }

    /// 查询某个 RTP 收流会话。
    ///
    /// 真实 ZLM（master）返回的是**扁平**结构，字段名也和 `RtpInfo` 不同：
    ///
    /// ```json
    /// {"code":0,"exist":true,"identifier":"<stream_id>","local_ip":"::",
    ///  "local_port":30052,"peer_ip":"172.18.0.1","peer_port":65288}
    /// ```
    ///
    /// 此前用 `ApiResponse<RtpInfo>`（要求 `data.stream_id`）解析 ——
    /// 真实 ZLM 上**恒定返回 None**，于是"流已存在就复用"这类判断永远走不通，
    /// 第二个观看者会拿到 `-300 This stream already exists` 而失败。
    pub async fn get_rtp_info(&self, stream_id: &str) -> Result<Option<RtpInfo>> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("stream_id", stream_id.to_string()),
        ];
        let raw: serde_json::Value = self.request("/index/api/getRtpInfo", &params).await?;
        Ok(parse_rtp_info(stream_id, &raw))
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

        let resp: serde_json::Value = self.request("/index/api/isRecording", &params).await?;
        Ok(exist_flag(&resp))
    }

    /// 列出某路流在 ZLM 上的 MP4 录像文件。
    ///
    /// 三个此前的**静默失效点**：
    /// 1. 路径大小写写错：真实 API 是 `/index/api/getMP4RecordFile`（MP4 全大写），
    ///    写成 `getMp4RecordFile` 直接 404 → 永远返回空列表；
    /// 2. 少传 `vhost`：ZLM 会回 `-300 Required parameter missed: "vhost", "app", "stream"`；
    /// 3. 响应结构想当然：真实返回是 `{"code":0,"data":{"rootPath":"<录像目录>","paths":[...]}}`
    ///    —— `paths` 在**不传 period** 时是**日期目录**，传了 period 才是文件名。
    ///    原来按 `data.list[{name,path,...}]` 解析，即使前两点修好也拿不到数据。
    ///
    /// `period`（本函数的 `path` 参数）为空时会先取日期目录再逐个展开成文件列表。
    pub async fn get_mp4_record_file(
        &self,
        app: &str,
        stream: &str,
        period: Option<&str>,
        _start_time: Option<&str>,
        _end_time: Option<&str>,
    ) -> Result<Vec<Mp4RecordFile>> {
        match period {
            Some(p) => self.list_mp4_files(app, stream, p).await,
            None => {
                let mut all = Vec::new();
                for day in self.list_mp4_periods(app, stream).await? {
                    all.extend(self.list_mp4_files(app, stream, &day).await?);
                }
                Ok(all)
            }
        }
    }

    /// 某个流的录像日期目录（`YYYY-MM-DD`）。
    pub async fn list_mp4_periods(&self, app: &str, stream: &str) -> Result<Vec<String>> {
        let resp = self.get_mp4_record_paths(app, stream, None).await?;
        Ok(resp.paths)
    }

    /// 某个日期目录下的录像文件。
    pub async fn list_mp4_files(&self, app: &str, stream: &str, period: &str) -> Result<Vec<Mp4RecordFile>> {
        let resp = self.get_mp4_record_paths(app, stream, Some(period)).await?;
        let root = resp.root_path.clone();
        Ok(resp
            .paths
            .into_iter()
            .map(|name| {
                // 文件名形如 `2026-09-12-22-09-45-0.mp4`，可反推录制起始时间
                let create_time = parse_mp4_file_name_time(&name).unwrap_or_default();
                Mp4RecordFile {
                    name: name.clone(),
                    size: 0,
                    create_time,
                    path: format!("{root}{name}"),
                    file_path: Some(root.clone()),
                    duration: None,
                }
            })
            .collect())
    }

    async fn get_mp4_record_paths(
        &self,
        app: &str,
        stream: &str,
        period: Option<&str>,
    ) -> Result<Mp4RecordPaths> {
        let mut params = vec![
            ("secret", self.secret.clone()),
            ("vhost", "__defaultVhost__".to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
        ];
        if let Some(p) = period {
            params.push(("period", p.to_string()));
        }
        let resp: ApiResponse<Mp4RecordPaths> =
            self.request("/index/api/getMP4RecordFile", &params).await?;
        resp.data.ok_or_else(|| {
            anyhow!(
                "getMP4RecordFile 无数据: code={} msg={}",
                resp.code,
                resp.msg.unwrap_or_default()
            )
        })
    }

    /// 删除一个录像文件。
    ///
    /// 真实 ZLM（master）**没有** `/index/api/deleteRecord`（实测 404），
    /// 正确的是 `/index/api/deleteRecordDirectory`，参数
    /// `vhost/app/stream/period(=日期目录)/file_name`。
    /// 传 `file_name` 时只删该文件；不传则删掉整个日期目录。
    pub async fn delete_record_file(
        &self,
        app: &str,
        stream: &str,
        period: &str,
        file_name: &str,
    ) -> Result<()> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("vhost", "__defaultVhost__".to_string()),
            ("app", app.to_string()),
            ("stream", stream.to_string()),
            ("period", period.to_string()),
            ("file_name", file_name.to_string()),
        ];
        #[derive(Deserialize)]
        struct Resp {
            code: i32,
            #[serde(default)]
            msg: Option<String>,
        }
        let resp: Resp = self
            .request("/index/api/deleteRecordDirectory", &params)
            .await?;
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

    /// 下发并**回读验证**：ZLM 各版本键名不同（例如 master 用
    /// `rtp_proxy.port_range`，旧版用 `rtp.port_range`），不存在的键
    /// `setServerConfig` 会返回 `code:0` 但什么也不做 —— 静默无效最难排查。
    ///
    /// 返回 `Ok(true)` 表示回读值已生效；`Ok(false)` 表示 ZLM 未接受该键
    /// （调用方应告警，而不是当成成功）。
    pub async fn set_server_config_verified(
        &self,
        secret: &str,
        key: &str,
        value: &str,
    ) -> Result<bool> {
        self.set_server_config(secret, key, value).await?;
        let cfg = self.get_server_config().await?;
        Ok(cfg.get(key).map(|v| v.trim() == value.trim()).unwrap_or(false))
    }

    pub async fn get_server_config(&self) -> Result<HashMap<String, String>> {
        let params = vec![("secret", self.secret.clone())];
        
        #[derive(Deserialize)]
        struct Resp {
            // ZLM 响应字段：仅用于反序列化（保留以完整映射 API）
            #[serde(rename = "api.apiDebug")]
            #[allow(dead_code)]
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
    /// 并发下发一批 `setServerConfig`，并给整批一个总预算。
    ///
    /// 为什么需要它：ZLM 可达时单次调用是毫秒级，串行循环看不出问题；
    /// 但 ZLM 不可达（或前面挂了反向代理返回 502）时，每次调用都要等满
    /// 客户端超时。实测 `POST /api/server/media_server/save` 在 ZLM 不健康时
    /// **串行**下发 11 个 hook 配置要 33 秒（最坏 30s × 11 = 5 分钟以上），
    /// 而 ZLM 的 `on_server_started` 回调里还有 13 + 2 + N 项配置要下发。
    ///
    /// 行为：并发下发；超出预算后取消剩余任务，并在返回的错误列表里
    /// 明确写出「超时 + 还有多少项未完成」，而不是无限等下去或假装成功。
    ///
    /// 返回：失败/超时项的说明文本（空表示全部成功）。
    pub async fn set_server_configs_batch(
        &self,
        secret: &str,
        items: Vec<(String, String)>,
        budget: std::time::Duration,
    ) -> Vec<String> {
        let mut set = tokio::task::JoinSet::new();
        for (key, value) in items {
            let client = self.clone();
            let secret = secret.to_string();
            set.spawn(async move {
                let r = client.set_server_config(&secret, &key, &value).await;
                (key, value, r)
            });
        }

        let mut errors = Vec::new();
        let deadline = tokio::time::Instant::now() + budget;
        loop {
            match tokio::time::timeout_at(deadline, set.join_next()).await {
                Err(_) => {
                    let pending = set.len();
                    set.abort_all();
                    errors.push(format!(
                        "下发 ZLM 配置超时（{:?}），仍有 {} 项未完成",
                        budget, pending
                    ));
                    break;
                }
                Ok(None) => break,
                Ok(Some(Ok((key, value, Err(e))))) => {
                    errors.push(format!("{}={}: {}", key, value, e));
                }
                Ok(Some(Ok((_key, _value, Ok(()))))) => {}
                Ok(Some(Err(join_err))) => {
                    errors.push(format!("ZLM 配置任务异常: {}", join_err));
                }
            }
        }
        errors
    }

    /// 下发一项 ZLM 配置。
    ///
    /// ZLMediaKit 的 `/index/api/setServerConfig` 只认**查询参数/表单**形式的
    /// `键=值`（它内部读的是 URL args），**不认** `{"key":..,"value":..}` 这种
    /// JSON body。此前这里发的就是 `{secret, key, value}` JSON：ZLM 忽略 body、
    /// 因为 secret 合法而返回 `code:0`，于是**所有 autoConfig 都静默无效** ——
    /// hook 地址（收不到任何事件）、`rtp.port_range`、`protocol.*`、
    /// `general.mediaServerId` 全都下发不进去。
    pub async fn set_server_config(&self, secret: &str, key: &str, value: &str) -> Result<()> {
        #[derive(Deserialize)]
        struct Resp {
            code: i32,
            #[serde(default)]
            msg: Option<String>,
        }
        let params = vec![
            ("secret", secret.to_string()),
            (key, value.to_string()),
        ];
        let resp: Resp = self.request("/index/api/setServerConfig", &params).await?;
        if resp.code == 0 {
            Ok(())
        } else {
            Err(anyhow!(
                "setServerConfig {key}={value} failed: code={} msg={}",
                resp.code,
                resp.msg.unwrap_or_default()
            ))
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
        #[allow(dead_code)]
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
        #[allow(dead_code)]
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
        #[allow(dead_code)]
        struct Resp { code: i32 }
        let resp: ApiResponse<Resp> = self.request("/index/api/startSendRtp", &params).await?;

        if resp.code != 0 {
            return Err(anyhow!("ZLM startSendRtp error: {}", resp.msg.unwrap_or_default()));
        }
        Ok(())
    }

    /// B3: 停止 ZLM 向某个上级平台的 SendRtp 推送 (stopSendRtp)
    /// 停止一路 SendRtp 推流。
    ///
    /// ZLM 的 `stopSendRtp` 允许用 `stream` **或** `ssrc` 选中会话；
    /// 两者都给最稳妥。此前调用方把 **ssrc** 传进了 `stream` 参数，
    /// ZLM 找不到名为该 ssrc 的流 ⇒ 推流从未真正停止（上游会一直收到 RTP）。
    pub async fn stop_send_rtp(
        &self,
        vhost: &str,
        app: &str,
        stream: &str,
    ) -> Result<()> {
        self.stop_send_rtp_ex(vhost, app, Some(stream), None).await
    }

    /// 同上，但可显式给出 `ssrc`（两者任一即可定位会话）。
    pub async fn stop_send_rtp_ex(
        &self,
        vhost: &str,
        app: &str,
        stream: Option<&str>,
        ssrc: Option<&str>,
    ) -> Result<()> {
        let mut params = vec![
            ("secret", self.secret.clone()),
            ("vhost", vhost.to_string()),
            ("app", app.to_string()),
        ];
        if let Some(s) = stream.filter(|s| !s.is_empty()) {
            params.push(("stream", s.to_string()));
        }
        if let Some(s) = ssrc.filter(|s| !s.is_empty()) {
            params.push(("ssrc", s.to_string()));
        }
        if stream.map(|s| s.is_empty()).unwrap_or(true) && ssrc.is_none() {
            return Err(anyhow!("stopSendRtp 需要 stream 或 ssrc 之一"));
        }

        #[derive(Deserialize)]
        #[allow(dead_code)]
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
        
        let resp: ApiResponse<Resp> = self.request("/index/api/getDownloadList", &params).await?;
        Ok(resp.data.map(|r| r.list).unwrap_or_default())
    }

    pub async fn stop_download(&self, file_name: &str) -> Result<()> {
        let params = vec![
            ("secret", self.secret.clone()),
            ("file_name", file_name.to_string()),
        ];

        #[derive(Deserialize)]
        #[allow(dead_code)]
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
/// 解析端口范围，**同时接受 ZLM 的 `start-end` 与历史配置的 `start,end`**。
///
/// 此前只认逗号：而 `gb_media_server.rtp_port_range` 里存的是 ZLM 风格的
/// `30000-30100`（与 `getServerConfig` 回读一致），于是下发一律失败。
pub fn parse_port_range(s: &str) -> Result<(u16, u16)> {
    let normalized = s.trim().replace('-', ",");
    let parts: Vec<&str> = normalized.split(',').map(str::trim).collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid port range: {} (expected 'start-end' or 'start,end')", s));
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

/// 依次尝试多个候选键名，**回读验证**后返回真正生效的那个键。
///
/// ZLM 各版本端口范围键名不同（`rtp_proxy.port_range` / `rtp.port_range`），
/// 不存在的键会返回 `code:0` 但什么都不做 —— 必须回读才能确认。
pub async fn set_rtp_port_range_verified(
    zlm: &ZlmClient,
    secret: &str,
    candidate_keys: &[&str],
    raw: &str,
) -> Result<String> {
    let (start, end) = parse_port_range(raw)?;
    let value = format!("{}-{}", start, end);
    let mut last_err = anyhow!("没有可用的端口范围配置键");
    for key in candidate_keys {
        match zlm.set_server_config_verified(secret, key, &value).await {
            Ok(true) => return Ok((*key).to_string()),
            Ok(false) => {
                last_err = anyhow!("{key} 下发后回读值不一致（该 ZLM 版本可能不支持）")
            }
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
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
        #[allow(dead_code)]
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
        #[allow(dead_code)]
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

    /// `isRecording` / `isMediaExist`：官方 ZLM 返回**顶层** `exist`，
    /// 不能用"要求 `data.exist`"的结构体去解析 —— 那样在真实 ZLM 上恒为
    /// false，只有 mock 上碰巧正确。
    #[test]
    fn exist_flag_accepts_flat_and_wrapped_shapes() {
        // 官方 ZLM 的扁平形态
        assert!(exist_flag(&serde_json::json!({"code": 0, "exist": true})));
        assert!(!exist_flag(&serde_json::json!({"code": 0, "exist": false})));
        // 部分分支/兼容实现包在 data 里
        assert!(exist_flag(
            &serde_json::json!({"code": 0, "data": {"exist": true}})
        ));
        // 旧 mock 用的 status 字段
        assert!(exist_flag(&serde_json::json!({"code": 0, "status": true})));
        assert!(exist_flag(
            &serde_json::json!({"code": 0, "data": {"status": true}})
        ));
        // 缺字段 / 非布尔 → false，绝不 panic
        assert!(!exist_flag(&serde_json::json!({"code": 0})));
        assert!(!exist_flag(&serde_json::json!({"code": -1})));
        assert!(!exist_flag(&serde_json::json!({"exist": "yes"})));
        assert!(!exist_flag(&serde_json::json!(null)));
    }

    /// 顶层优先：顶层给了结论就不看 data，避免两种形态给出矛盾答案时行为漂移。
    #[test]
    fn exist_flag_prefers_top_level() {
        assert!(!exist_flag(
            &serde_json::json!({"exist": false, "data": {"exist": true}})
        ));
    }

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

            // setServerConfig 走 GET + 查询参数（ZLM 不认 JSON body）
            Mock::given(method("GET"))
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
            // 端口范围走**查询参数**（ZLM 只认 URL args，不认 {key,value} JSON body）
            let url = received[0].url.as_str();
            assert!(
                url.contains("rtp.port_range=30000-30200"),
                "expected rtp.port_range=30000-30200 in query, got: {url}"
            );
        }
    }
}

#[cfg(test)]
mod port_range_tests {
    use super::parse_port_range;

    /// ZLM 自己的格式是 `start-end`，历史配置写过 `start,end`，两种都要认。
    #[test]
    fn test_parse_port_range_accepts_both_separators() {
        assert_eq!(parse_port_range("30000-30100").unwrap(), (30000, 30100));
        assert_eq!(parse_port_range("30000,30100").unwrap(), (30000, 30100));
        assert_eq!(parse_port_range(" 30000 - 30100 ").unwrap(), (30000, 30100));
        assert!(parse_port_range("30000").is_err());
        assert!(parse_port_range("a-b").is_err());
    }
}

#[cfg(test)]
mod rtp_info_tests {
    use super::parse_rtp_info;

    /// 真实 ZLM master 的扁平响应必须能解析出来。
    #[test]
    fn test_parse_rtp_info_flat_response() {
        let raw = serde_json::json!({
            "code": 0,
            "exist": true,
            "identifier": "34020000001320000001_34020000001310000001",
            "local_ip": "::",
            "local_port": 30052,
            "peer_ip": "172.18.0.1",
            "peer_port": 65288
        });
        let info = parse_rtp_info("fallback", &raw).expect("扁平响应应解析出 RTP 信息");
        assert_eq!(info.stream_id, "34020000001320000001_34020000001310000001");
        assert_eq!(info.local_port, 30052);
        assert_eq!(info.peer_ip, "172.18.0.1");
        assert_eq!(info.peer_port, 65288);
    }

    #[test]
    fn test_parse_rtp_info_absent_stream_is_none() {
        let raw = serde_json::json!({"code": 0, "exist": false});
        assert!(parse_rtp_info("s", &raw).is_none());
        let empty = serde_json::json!({"code": 0, "data": null});
        assert!(parse_rtp_info("s", &empty).is_none());
    }

    #[test]
    fn test_parse_rtp_info_nested_response() {
        let raw = serde_json::json!({
            "code": 0,
            "data": {
                "stream_id": "s", "ssrc": "1", "peer_ip": "1.2.3.4",
                "peer_port": 5, "local_port": 6, "alive_second": 7, "rtt": 8
            }
        });
        let info = parse_rtp_info("fallback", &raw).expect("嵌套响应也要兼容");
        assert_eq!(info.stream_id, "s");
        assert_eq!(info.local_port, 6);
    }
}
