//! `/api/cloud/record/*` extras — collect toggles, list-url, zip packaging.
//! These complement the CRUD already in `src/handlers/cloud_record.rs`.

use axum::{extract::{Query, State}, Json};
use serde::Deserialize;

use crate::db;
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
    pub device_id: Option<String>,
    #[serde(default)]
    pub channel_id: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct ZipQuery {
    pub ids: Option<String>, // comma-separated CloudRecord ids
}

/// GET /api/cloud/record/collect/add?id=<i64>
pub async fn collect_add(
    State(state): State<AppState>,
    Query(q): Query<CollectQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let id = match q.id {
        Some(i) if i > 0 => i,
        _ => return Json(WVPResult::error("missing id")),
    };
    match db::cloud_record::set_collect(&state.pool, id, true).await {
        Ok(true) => Json(WVPResult::success(serde_json::json!({"id": id, "collect": true}))),
        Ok(false) => Json(WVPResult::error("record not found")),
        Err(e) => Json(WVPResult::error(format!("DB error: {}", e))),
    }
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
        let path = r.file_path.clone().unwrap_or_default();
        let url = if path.is_empty() {
            String::new()
        } else {
            format!("/record/{}", path)
        };
        serde_json::json!({
            "id": r.id,
            "app": r.app,
            "stream": r.stream,
            "fileName": r.file_name,
            "url": url,
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

/// 打包核心：按 id 取记录 → 定位磁盘文件 → 写 ZIP。
///
/// 刻意不接收 `AppState`，只依赖 `pool` 与路径，便于用真实 SQLite 做端到端测试。
/// 不可用的记录进入 `skipped` 如实回报；全部不可用时返回 `Err`（而不是假装成功）。
pub(crate) async fn build_cloud_record_zip(
    pool: &db::Pool,
    ids: &[i64],
    record_root: Option<&std::path::Path>,
    download_dir: &std::path::Path,
) -> Result<ZipBuildOutcome, String> {
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    let mut skipped: Vec<serde_json::Value> = Vec::new();

    for id in ids {
        match db::cloud_record::get_by_id(pool, *id).await {
            Ok(Some(rec)) => {
                let display = rec
                    .file_name
                    .clone()
                    .unwrap_or_else(|| format!("record-{}.mp4", rec.id));
                match resolve_record_file(&rec, record_root) {
                    Some(path) => files.push((format!("record/{}", display), path)),
                    None => skipped.push(serde_json::json!({
                        "id": rec.id,
                        "fileName": display,
                        "status": "file_missing",
                        "reason": "记录中的 file_path 在本机不存在；若 ZLM 挂载点不同，可设置 server.record_root",
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

    let outcome = match build_cloud_record_zip(&state.pool, &ids, record_root.as_deref(), &dir).await
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

    /// 建一个跑过生产 schema 的内存 SQLite
    #[cfg(feature = "sqlite")]
    async fn sqlite_pool_with_schema() -> db::Pool {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;

        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("sqlite in-memory pool");

        let sql = include_str!("../../database/init-sqlite-2.7.4.sql");
        let cleaned: String = sql
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !t.is_empty() && !t.starts_with("--")
            })
            .collect::<Vec<_>>()
            .join("\n");
        for raw in cleaned.split(';') {
            let stmt = raw.trim();
            if stmt.is_empty() {
                continue;
            }
            let upper = stmt.to_uppercase();
            if !upper.starts_with("CREATE") && !upper.starts_with("INSERT") {
                continue;
            }
            sqlx::query(stmt).execute(&pool).await.unwrap_or_else(|e| {
                panic!("init SQL failed: {} | stmt: {}", e, &stmt[..80.min(stmt.len())])
            });
        }
        pool
    }

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
        let outcome = build_cloud_record_zip(&pool, &[id1, id2], None, &out_dir)
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
        let err = build_cloud_record_zip(&pool, &[id], None, &out_dir)
            .await
            .expect_err("文件缺失时必须报错");
        assert!(err.contains("没有可打包"), "错误信息应说明原因: {}", err);

        // 2) 记录本身不存在
        let err2 = build_cloud_record_zip(&pool, &[999_999], None, &out_dir)
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

        let outcome = build_cloud_record_zip(&pool, &[ok_id, bad_id], None, &work)
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
