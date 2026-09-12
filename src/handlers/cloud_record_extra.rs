//! `/api/cloud/record/*` extras — collect toggles, list-url, zip packaging.
//! These complement the CRUD already in `src/handlers/cloud_record.rs`.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db;
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::AppState;

#[derive(Deserialize, Default)]
pub struct CollectQuery {
    pub id: Option<i64>,
}

#[derive(Deserialize, Default)]
pub struct ListUrlQuery {
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(default)]
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct ZipQuery {
    pub ids: Option<String>, // comma-separated CloudRecord ids
}

/// GET /api/cloud/record/collect/delete?id=<i64>
pub async fn collect_delete(
    State(state): State<AppState>,
    Query(q): Query<CollectQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let id = match q.id {
        Some(i) if i > 0 => i,
        _ => return Json(WVPResult::error("missing id")),
    };
    match db::cloud_record::set_collect(&state.pool, id, false).await {
        Ok(true) => Json(WVPResult::success(serde_json::json!({"id": id, "collect": false}))),
        Ok(false) => Json(WVPResult::error("record not found")),
        Err(e) => Json(WVPResult::error(format!("DB error: {}", e))),
    }
}

/// GET /api/cloud/record/list-url?device_id=&channel_id=
pub async fn list_url(
    State(state): State<AppState>,
    Query(q): Query<ListUrlQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let page = q.page.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(15);
    let kw = q.device_id.as_deref().or(q.channel_id.as_deref());
    let records = db::cloud_record::list_paged(
        &state.pool, kw, None, None, None, None,
        page as i64, count as i64,
    ).await.unwrap_or_default();
    let total = db::cloud_record::count_all(
        &state.pool, kw, None, None, None, None,
    ).await.unwrap_or(0);
    let urls: Vec<serde_json::Value> = records.iter().map(|r| {
        // 2026-09-11：此前这里给出 `/record/<path>`，但**没有对应路由**，
        // 点开必然 404。现改为真实可用的下载端点（支持 Range）。
        let has_file = r.file_path.as_deref().map(|p| !p.is_empty()).unwrap_or(false);
        serde_json::json!({
            "id": r.id,
            "app": r.app,
            "stream": r.stream,
            "fileName": r.file_name,
            "url": if has_file { format!("/api/cloud/record/download/{}", r.id) } else { String::new() },
            "startTime": r.start_time,
            "endTime": r.end_time,
            "duration": r.time_len,
            "fileSize": r.file_size,
        })
    }).collect();
    Json(WVPResult::success(serde_json::json!({
        "list": urls,
        "total": total,
        "page": page,
        "count": count,
    })))
}

/// 解析 `ids=1,2,3`（容忍空格、空段，过滤非正数）
fn parse_zip_ids(raw: &str) -> Vec<i64> {
    raw.split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .filter(|v| *v > 0)
        .collect()
}

/// 在本机定位某条云录像对应的文件。
///
/// 依次尝试：
/// 1. `file_path` 原样（ZLM 与 GBServer 共享文件系统的常见情形）
/// 2. `record_root / <file_name>`
/// 3. `record_root / <folder> / <file_name>`（容器内挂载点不同但共享卷）
fn resolve_record_file(
    rec: &db::cloud_record::CloudRecord,
    record_root: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    let file_name = rec.file_name.as_deref().filter(|s| !s.is_empty());
    let folder = rec.folder.as_deref().filter(|s| !s.is_empty());

    if let Some(p) = rec.file_path.as_deref().filter(|s| !s.is_empty()) {
        let pb = std::path::PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
        if let Some(root) = record_root {
            if let Some(name) = pb.file_name() {
                let cand = root.join(name);
                if cand.is_file() {
                    return Some(cand);
                }
            }
            if let (Some(folder), Some(name)) = (folder, pb.file_name()) {
                let cand = root.join(folder).join(name);
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
    }

    // file_path 缺失时，退回用 folder / file_name 在 record_root 下拼
    if let (Some(root), Some(name)) = (record_root, file_name) {
        let mut cand = root.to_path_buf();
        if let Some(folder) = folder {
            cand = cand.join(folder);
        }
        cand = cand.join(name);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

/// 生成不可枚举的下载令牌：静态目录是公开可读的，用随机段避免被遍历猜测
fn random_token() -> String {
    use rand::RngCore;
    let mut b = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut b);
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

/// ZIP 打包结果
#[derive(Debug)]
pub(crate) struct ZipBuildOutcome {
    pub token: String,
    pub file_name: String,
    pub out_path: std::path::PathBuf,
    pub entries: Vec<crate::archive::ZipEntry>,
    pub skipped: Vec<serde_json::Value>,
}

/// 从 ZLM 的 HTTP 服务把录像文件抓到本地缓存目录。
///
/// 容器化部署（docker compose）下 ZLM 与后端**不共享文件系统**，
/// `record.file_path` 是容器内路径（`/opt/media/bin/www/record/...`），
/// 本机 `is_file()` 永远为假 —— 于是"打包下载"必然全部 `skipped`。
/// 这里改成走 ZLM 的静态 HTTP 路径（`http://ip:port/record/...`）把文件取回来。
///
/// 返回本地缓存文件路径。
async fn fetch_record_from_zlm(
    state: &AppState,
    rec: &db::cloud_record::CloudRecord,
    cache_dir: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let media_server_id = rec
        .media_server_id
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let (Some(zlm), Some(file_name)) = (
        state.get_zlm_client(Some(&media_server_id)),
        rec.file_name.clone().filter(|s| !s.is_empty()),
    ) else {
        return Err("缺少媒体节点或文件名".to_string());
    };

    // 与列表接口同一套相对路径推导
    let rel = crate::handlers::stub::cloud_record_rel_path_for(
        rec.file_path.as_deref(),
        &rec.app,
        &rec.stream,
        &file_name,
    );
    if rel.is_empty() {
        return Err("无法推导录像相对路径".to_string());
    }
    let url = format!("http://{}:{}/{}", zlm.ip, zlm.http_port, rel);

    std::fs::create_dir_all(cache_dir).map_err(|e| format!("创建缓存目录失败: {e}"))?;
    let out = cache_dir.join(format!("{}-{}", rec.id, file_name));
    if out.is_file() {
        return Ok(out);
    }

    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("从 ZLM 拉取 {url} 失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("从 ZLM 拉取 {url} 返回 HTTP {}", resp.status()));
    }
    let mut resp = resp;
    let mut file = tokio::fs::File::create(&out)
        .await
        .map_err(|e| format!("创建缓存文件失败: {e}"))?;
    use tokio::io::AsyncWriteExt;
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("读取 ZLM 响应失败: {e}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写缓存文件失败: {e}"))?;
    }
    file.flush().await.map_err(|e| format!("flush 失败: {e}"))?;
    Ok(out)
}

/// 打包核心：按 id 取记录 → 定位磁盘文件 → 写 ZIP。
///
/// 刻意不接收 `AppState`，只依赖 `pool` 与路径，便于用真实 SQLite 做端到端测试。
/// 不可用的记录进入 `skipped` 如实回报；全部不可用时返回 `Err`（而不是假装成功）。
pub(crate) async fn build_cloud_record_zip(
    state: Option<&AppState>,
    pool: &db::Pool,
    ids: &[i64],
    record_root: Option<&std::path::Path>,
    download_dir: &std::path::Path,
) -> Result<ZipBuildOutcome, String> {
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    let mut skipped: Vec<serde_json::Value> = Vec::new();
    let cache_dir = download_dir.join(".cloud-record-cache");

    for id in ids {
        match db::cloud_record::get_by_id(pool, *id).await {
            Ok(Some(rec)) => {
                let display = rec
                    .file_name
                    .clone()
                    .unwrap_or_else(|| format!("record-{}.mp4", rec.id));
                let local = resolve_record_file(&rec, record_root);
                // 本机没有（容器化 ZLM）→ 从媒体服务器的 HTTP 服务拉回来再打包
                let resolved = match (local, state) {
                    (Some(p), _) => Some(p),
                    (None, Some(st)) => match fetch_record_from_zlm(st, &rec, &cache_dir).await {
                        Ok(p) => Some(p),
                        Err(e) => {
                            tracing::warn!("打包时拉取录像失败 id={}: {e}", rec.id);
                            None
                        }
                    },
                    (None, None) => None,
                };
                match resolved {
                    Some(path) => files.push((format!("record/{}", display), path)),
                    None => skipped.push(serde_json::json!({
                        "id": rec.id,
                        "fileName": display,
                        "status": "file_missing",
                        "reason": "本机找不到该文件，且无法从媒体服务器拉取；可设置 server.record_root 或检查 ZLM 是否可达",
                    })),
                }
            }
            Ok(None) => skipped.push(serde_json::json!({
                "id": id, "status": "record_not_found"
            })),
            Err(e) => skipped.push(serde_json::json!({
                "id": id, "status": "db_error", "reason": e.to_string()
            })),
        }
    }

    if files.is_empty() {
        return Err(format!(
            "没有可打包的录像文件：{} 条记录全部不可用",
            ids.len()
        ));
    }

    let token = format!("{}-{}", chrono::Utc::now().timestamp(), random_token());
    let file_name = format!("cloud-record-{}.zip", token);
    let out_path = download_dir.join(&file_name);
    let entries =
        crate::archive::write_zip_stored(&files, &out_path).map_err(|e| format!("打包失败: {}", e))?;

    Ok(ZipBuildOutcome {
        token,
        file_name,
        out_path,
        entries,
        skipped,
    })
}

/// GET /api/cloud/record/download/zip?ids=1,2,3
///
/// 真正把选中的云录像文件打包成 ZIP，落到 `server.download_dir`，
/// 并返回可直接下载的 URL 与每个文件的处理结果。
///
/// 2026-09-11：此前该端点是**编造成功** —— 返回一个凭空拼出的 `taskId`
/// 与 `status: "queued"`，声称「后台完成后可通过 taskId 查询」，但实际上既没有
/// 创建任何任务、也没有对应的查询端点。现改为**同步真实打包**：
/// 录像多为 MP4/PS 已压缩格式，使用 ZIP stored 方式（见 `crate::archive`），
/// 无需引入压缩依赖，速度也更快。
pub async fn download_zip(
    State(state): State<AppState>,
    Query(q): Query<ZipQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let ids = parse_zip_ids(q.ids.as_deref().unwrap_or_default());
    if ids.is_empty() {
        return Json(WVPResult::error("missing ids"));
    }

    let record_root = state
        .config
        .server
        .record_root
        .as_deref()
        .map(std::path::PathBuf::from);
    let dir = state.config.server.effective_download_dir();

    let outcome = match build_cloud_record_zip(
        Some(&state),
        &state.pool,
        &ids,
        record_root.as_deref(),
        &dir,
    )
    .await
    {
        Ok(o) => o,
        Err(msg) => return Json(WVPResult::error(msg)),
    };

    let total_bytes: u64 = outcome.entries.iter().map(|z| z.size).sum();
    Json(WVPResult::success(serde_json::json!({
        "taskId": outcome.token,
        "ids": ids,
        "status": "done",
        "fileName": outcome.file_name,
        "url": format!("/downloads/{}", outcome.file_name),
        "filePath": outcome.out_path.to_string_lossy(),
        "fileCount": outcome.entries.len(),
        "totalBytes": total_bytes,
        "list": outcome.entries.iter().map(|z| serde_json::json!({
            "name": z.name,
            "size": z.size,
            "crc32": format!("{:08x}", z.crc32),
        })).collect::<Vec<_>>(),
        "skipped": outcome.skipped,
        "msg": format!("已打包 {} 个文件", outcome.entries.len()),
    })))
}

/// GET /api/cloud/record/zip?ids=1,2,3 — alias of download/zip
pub async fn zip(
    State(state): State<AppState>,
    Query(q): Query<ZipQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    download_zip(State(state), Query(q)).await
}

/// GET /api/cloud/record/download/:id — 下载/播放单条录像文件
///
/// 2026-09-11 新增：`list_url` 与告警抓拍此前给出的 `/record/<path>` URL
/// **并没有对应的路由**，即返回的地址点开必然 404。此端点提供真实可用的下载地址。
///
/// 内部用 `tower_http::services::ServeFile`，因此**自动支持 HTTP Range** ——
/// `<video>` 标签才可能拖动进度条。
/// 把 ZLM 的录像文件 HTTP 响应原样代理给客户端（支持 Range 透传）。
async fn proxy_record_from_zlm(
    state: &AppState,
    rec: &db::cloud_record::CloudRecord,
    headers: &axum::http::HeaderMap,
) -> Result<axum::response::Response, AppError> {
    use axum::body::Body;
    use axum::http::{header, Response, StatusCode};

    let media_server_id = rec
        .media_server_id
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let Some(zlm) = state.get_zlm_client(Some(&media_server_id)) else {
        return Err(AppError::business(
            ErrorCode::Error404,
            format!("媒体节点不可用: {media_server_id}"),
        ));
    };
    let file_name = rec.file_name.clone().unwrap_or_default();
    let rel = crate::handlers::stub::cloud_record_rel_path_for(
        rec.file_path.as_deref(),
        &rec.app,
        &rec.stream,
        &file_name,
    );
    if rel.is_empty() {
        return Err(AppError::business(
            ErrorCode::Error404,
            "无法推导录像文件路径".to_string(),
        ));
    }
    let url = format!("http://{}:{}/{}", zlm.ip, zlm.http_port, rel);

    let client = reqwest::Client::new();
    let mut req = client.get(&url);
    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        // axum 与 reqwest 各自依赖不同版本的 http crate，这里按字符串转换
        req = req.header("range", range.to_string());
    }
    let resp = req
        .send()
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("请求 ZLM 失败: {e}")))?;
    let status = resp.status();
    // 注意：axum 与 reqwest 依赖不同版本的 http crate，头名字符串比常量更稳
    let header_str = |name: &str| -> Option<String> {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let content_type = header_str("content-type").unwrap_or_else(|| "video/mp4".to_string());
    let content_range = header_str("content-range");
    let content_length = header_str("content-length");

    let mut out = Response::builder()
        .status(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK))
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCEPT_RANGES, "bytes");
    if let Some(v) = content_range {
        out = out.header(header::CONTENT_RANGE, v);
    }
    if let Some(v) = content_length {
        out = out.header(header::CONTENT_LENGTH, v);
    }
    let stream = resp.bytes_stream();
    Ok(out
        .body(Body::from_stream(stream))
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("构造响应失败: {e}")))?)
}

pub async fn download_file(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response, AppError> {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let rec = db::cloud_record::get_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::business(ErrorCode::Error404, format!("录像记录不存在: {}", id)))?;

    let record_root = state
        .config
        .server
        .record_root
        .as_deref()
        .map(std::path::PathBuf::from);
    // 本机没有该文件时（容器化部署最常见）**代理 ZLM 的 HTTP 响应**，
    // 而不是 404 —— 否则 /api/cloud/record/download/:id 与 list-url 给出的
    // url 全是死链。Range 头一起透传，视频才能拖动进度条。
    let local = resolve_record_file(&rec, record_root.as_deref());
    let Some(path) = local else {
        return proxy_record_from_zlm(&state, &rec, &headers).await;
    };

    // 把 Range 头透传给 ServeFile，以获得 206 分片响应
    let mut req = Request::builder()
        .uri("/")
        .body(Body::empty())
        .expect("build request");
    if let Some(range) = headers.get(axum::http::header::RANGE) {
        req.headers_mut()
            .insert(axum::http::header::RANGE, range.clone());
    }

    let res = ServiceExt::oneshot(tower_http::services::ServeFile::new(&path), req)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("读取录像文件失败: {}", e)))?;

    // ServeFile 的响应体是 ServeFileSystemResponseBody，需转成 axum 的 Body
    Ok(res.map(Body::new))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(
        file_path: Option<&str>,
        folder: Option<&str>,
        file_name: Option<&str>,
    ) -> db::cloud_record::CloudRecord {
        db::cloud_record::CloudRecord {
            id: 1,
            app: "record".into(),
            stream: "s".into(),
            call_id: None,
            start_time: None,
            end_time: None,
            media_server_id: None,
            server_id: None,
            file_name: file_name.map(|s| s.to_string()),
            folder: folder.map(|s| s.to_string()),
            file_path: file_path.map(|s| s.to_string()),
            collect: None,
            file_size: None,
            time_len: None,
        }
    }

    #[test]
    fn test_parse_zip_ids_valid() {
        assert_eq!(parse_zip_ids("10,20,30"), vec![10, 20, 30]);
        assert_eq!(parse_zip_ids(" 1 , 2 "), vec![1, 2]);
    }

    #[test]
    fn test_parse_zip_ids_filters_garbage_and_non_positive() {
        // 非数字、空段、0 与负数都会被过滤 —— 避免用无效 id 去查库
        assert_eq!(parse_zip_ids(""), Vec::<i64>::new());
        assert_eq!(parse_zip_ids("abc"), Vec::<i64>::new());
        assert_eq!(parse_zip_ids("1,,2"), vec![1, 2]);
        assert_eq!(parse_zip_ids("0,-5,7"), vec![7]);
    }

    #[test]
    fn test_resolve_record_file_prefers_direct_file_path() {
        let dir = std::env::temp_dir().join(format!("gbzip-r1-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.mp4");
        std::fs::write(&f, b"x").unwrap();

        let r = rec(Some(f.to_str().unwrap()), None, Some("a.mp4"));
        assert_eq!(resolve_record_file(&r, None), Some(f.clone()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_record_file_falls_back_to_record_root() {
        let root = std::env::temp_dir().join(format!("gbzip-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("b.mp4"), b"y").unwrap();

        // file_path 指向一个本机不存在的 ZLM 侧路径（典型容器挂载差异）
        let r = rec(
            Some("/zlm/media/record/b.mp4"),
            None,
            Some("b.mp4"),
        );
        assert_eq!(
            resolve_record_file(&r, Some(root.as_path())),
            Some(root.join("b.mp4"))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_resolve_record_file_uses_folder_under_root() {
        let root = std::env::temp_dir().join(format!("gbzip-root2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("2026-09-11")).unwrap();
        std::fs::write(root.join("2026-09-11").join("c.mp4"), b"z").unwrap();

        let r = rec(
            Some("/zlm/record/2026-09-11/c.mp4"),
            Some("2026-09-11"),
            Some("c.mp4"),
        );
        assert_eq!(
            resolve_record_file(&r, Some(root.as_path())),
            Some(root.join("2026-09-11").join("c.mp4"))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_resolve_record_file_returns_none_when_absent() {
        // 文件确实不存在时必须返回 None（上层据此如实回报 file_missing，而不是假装成功）
        let r = rec(
            Some("/definitely/not/here/none.mp4"),
            None,
            Some("none.mp4"),
        );
        assert_eq!(resolve_record_file(&r, None), None);
        assert_eq!(resolve_record_file(&r, Some(std::path::Path::new("/tmp"))), None);
    }

    #[test]
    fn test_random_token_is_hex_and_varies() {
        let a = random_token();
        let b = random_token();
        assert_eq!(a.len(), 16, "8 字节应为 16 位十六进制");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b, "两次令牌不应相同");
    }

    #[test]
    fn test_list_url_query_defaults() {
        let q = ListUrlQuery::default();
        assert_eq!(q.page, None);
        assert_eq!(q.count, None);
    }

    #[test]
    fn test_zip_query_struct_field_default() {
        let q = ZipQuery::default();
        assert!(q.ids.is_none());
    }

    // ============ 端到端：真实 SQLite + 真实磁盘文件 + 真实 ZIP ============

    #[cfg(feature = "sqlite")]
    use crate::test_support::sqlite_pool_with_schema;

    #[cfg(feature = "sqlite")]
    fn insert_cfg(
        file_name: &str,
        folder: &str,
        file_path: &str,
    ) -> db::cloud_record::CloudRecordInsert {
        db::cloud_record::CloudRecordInsert {
            app: "record".into(),
            stream: "s".into(),
            call_id: None,
            start_time: Some(1),
            end_time: Some(2),
            media_server_id: None,
            server_id: None,
            file_name: Some(file_name.into()),
            folder: Some(folder.into()),
            file_path: Some(file_path.into()),
            file_size: None,
            time_len: Some(1.0),
        }
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_build_cloud_record_zip_end_to_end() {
        let pool = sqlite_pool_with_schema().await;

        let work = std::env::temp_dir().join(format!("gbzip-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let rec_dir = work.join("record");
        let out_dir = work.join("out");
        std::fs::create_dir_all(&rec_dir).unwrap();
        std::fs::create_dir_all(&out_dir).unwrap();

        let f1 = rec_dir.join("cam1.mp4");
        let f2 = rec_dir.join("cam2.mp4");
        std::fs::write(&f1, b"RAWDATA-ONE").unwrap();
        std::fs::write(&f2, b"RAWDATA-TWO-LONGER").unwrap();

        let id1 = db::cloud_record::insert(
            &pool,
            &insert_cfg("cam1.mp4", &rec_dir.to_string_lossy(), &f1.to_string_lossy()),
        )
        .await
        .expect("insert record 1");
        let id2 = db::cloud_record::insert(
            &pool,
            &insert_cfg("cam2.mp4", &rec_dir.to_string_lossy(), &f2.to_string_lossy()),
        )
        .await
        .expect("insert record 2");

        // 真实执行打包
        let outcome = build_cloud_record_zip(None, &pool, &[id1, id2], None, &out_dir)
            .await
            .expect("打包应成功");

        assert_eq!(outcome.entries.len(), 2);
        assert!(outcome.out_path.is_file(), "ZIP 必须真实落盘");
        assert!(outcome.skipped.is_empty(), "两条记录都可用，不应有 skipped");
        assert_eq!(outcome.file_name, format!("cloud-record-{}.zip", outcome.token));

        // 用独立读取器校验归档内容
        let back = crate::archive::read_zip_stored(&outcome.out_path)
            .expect("产出的 ZIP 应可被解析且结构自洽");
        assert_eq!(back.len(), 2);
        let d1 = back
            .iter()
            .find(|(n, _)| n == "record/cam1.mp4")
            .expect("应含 cam1");
        assert_eq!(d1.1, b"RAWDATA-ONE");
        let d2 = back
            .iter()
            .find(|(n, _)| n == "record/cam2.mp4")
            .expect("应含 cam2");
        assert_eq!(d2.1, b"RAWDATA-TWO-LONGER");

        let _ = std::fs::remove_dir_all(&work);
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_build_cloud_record_zip_all_unavailable_is_error() {
        // 关键回归保护：以前这里会返回编造的 taskId + "queued"。
        // 现在文件/记录都不可用时必须**明确报错**，不允许假成功。
        let pool = sqlite_pool_with_schema().await;
        let out_dir = std::env::temp_dir().join(format!("gbzip-e2e2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out_dir);
        std::fs::create_dir_all(&out_dir).unwrap();

        // 1) 记录存在但磁盘文件不存在
        let id = db::cloud_record::insert(
            &pool,
            &insert_cfg("ghost.mp4", "/no/such/dir", "/no/such/dir/ghost.mp4"),
        )
        .await
        .unwrap();
        let err = build_cloud_record_zip(None, &pool, &[id], None, &out_dir)
            .await
            .expect_err("文件缺失时必须报错");
        assert!(err.contains("没有可打包"), "错误信息应说明原因: {}", err);

        // 2) 记录本身不存在
        let err2 = build_cloud_record_zip(None, &pool, &[999_999], None, &out_dir)
            .await
            .expect_err("记录不存在时必须报错");
        assert!(err2.contains("没有可打包"), "错误信息应说明原因: {}", err2);

        // 3) 不应留下任何 .zip 产物
        let leftovers: Vec<_> = std::fs::read_dir(&out_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".zip"))
            .collect();
        assert!(leftovers.is_empty(), "失败时不应产出 ZIP 文件");

        let _ = std::fs::remove_dir_all(&out_dir);
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_build_cloud_record_zip_partial_success_reports_skipped() {
        // 一条可用 + 一条文件缺失 → 打包成功 1 个，skipped 如实列出 1 条
        let pool = sqlite_pool_with_schema().await;
        let work = std::env::temp_dir().join(format!("gbzip-e2e3-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).unwrap();

        let good = work.join("good.mp4");
        std::fs::write(&good, b"GOOD").unwrap();
        let ok_id = db::cloud_record::insert(
            &pool,
            &insert_cfg("good.mp4", &work.to_string_lossy(), &good.to_string_lossy()),
        )
        .await
        .unwrap();
        let bad_id = db::cloud_record::insert(
            &pool,
            &insert_cfg("bad.mp4", "/nope", "/nope/bad.mp4"),
        )
        .await
        .unwrap();

        let outcome = build_cloud_record_zip(None, &pool, &[ok_id, bad_id], None, &work)
            .await
            .expect("有一条可用即应成功");

        assert_eq!(outcome.entries.len(), 1);
        assert_eq!(outcome.skipped.len(), 1, "缺失的那条必须出现在 skipped");
        assert_eq!(outcome.skipped[0]["status"], "file_missing");
        assert_eq!(outcome.skipped[0]["id"], bad_id);

        let back = crate::archive::read_zip_stored(&outcome.out_path).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].0, "record/good.mp4");

        let _ = std::fs::remove_dir_all(&work);
    }
}
