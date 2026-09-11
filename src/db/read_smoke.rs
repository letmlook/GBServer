//! DB 读取路径冒烟测试
//!
//! ## 为什么需要它
//!
//! `sqlx::FromRow` 的结构体如果与 `SELECT` 的列清单不一致，**编译期完全发现不了**，
//! 只有真正解码到一行数据时才会报 `no column found for name: <field>`。
//! 空表上的 `fetch_all` 不会解码任何行，因此**空表测试也抓不到**。
//!
//! 2026-09-11 实测教训：`db::common_channel` 的 4 个函数因为列清单只有 12/32 列，
//! 导致所有走 `lookup_channel_and_send` 的 PTZ/预置位/雨刷等约 20 个端点
//! 在运行时全部返回「数据库错误」。
//!
//! 本测试的做法：schema 建好后**每个表插入至少一行**，再调用各读取函数并断言成功。
//! 任何列清单漂移都会在这里立即暴露。

#![cfg(all(test, feature = "sqlite"))]

use crate::db;
use crate::test_support::sqlite_pool_with_schema;

/// 播种最小可用数据（只填必填列，其余依赖 schema 默认值）
async fn seed(pool: &db::Pool) {
    let stmts: &[&str] = &[
        "INSERT INTO gb_media_server (id, status) VALUES ('zlm-a', 1)",
        "INSERT INTO gb_device (device_id, name, on_line, create_time, update_time) \
         VALUES ('34020000001320000001', 'cam-1', 1, '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        "INSERT INTO gb_device_channel \
         (device_id, name, gb_device_id, civil_code, status, data_type, data_device_id, \
          longitude, latitude, parent_id, create_time, update_time) \
         VALUES ('34020000001320000001', 'ch-1', '34020000001310000001', '340200', 'ON', 0, 0, \
                 118.7, 32.0, '0', '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        "INSERT INTO gb_cloud_record (app, stream, start_time, end_time, file_name, file_path) \
         VALUES ('record', '34020000001320000001_34020000001310000001', 1, 2, 'a.mp4', '/tmp/a.mp4')",
        "INSERT INTO gb_device_alarm (device_id, channel_id, create_time) \
         VALUES ('34020000001320000001', '34020000001310000001', '2026-01-01 00:00:00')",
        "INSERT INTO gb_device_mobile_position (device_id, channel_id) \
         VALUES ('34020000001320000001', '34020000001310000001')",
        "INSERT INTO gb_stream_proxy (id, app, stream, create_time, update_time) \
         VALUES (1, 'proxy', 's1', '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        "INSERT INTO gb_stream_push (id, app, stream, create_time, update_time) \
         VALUES (1, 'push', 's1', '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
    ];
    for sql in stmts {
        sqlx::query(sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("seed failed: {} | {}", e, sql));
    }
}

/// 媒体服务器：列清单必须覆盖 `MediaServer` 全部字段
#[tokio::test]
async fn smoke_media_server_reads() {
    let pool = sqlite_pool_with_schema().await;
    seed(&pool).await;
    db::media_server::list_media_servers(&pool).await.expect("list_media_servers");
    db::media_server::get_media_server_by_id(&pool, "zlm-a")
        .await
        .expect("get_media_server_by_id");
    db::media_server::get_default_server(&pool).await.expect("get_default_server");
    db::media_server::list_online_servers(&pool).await.expect("list_online_servers");
}

/// 设备 / 通道：`Device` 与 `DeviceChannel` 字段多，最容易漂移
#[tokio::test]
async fn smoke_device_and_channel_reads() {
    let pool = sqlite_pool_with_schema().await;
    seed(&pool).await;

    db::device::get_device_by_device_id(&pool, "34020000001320000001")
        .await
        .expect("get_device_by_device_id");
    db::device::list_devices_paged(&pool, 1, 10, None, None)
        .await
        .expect("list_devices_paged");
    db::device::count_devices(&pool, None, None).await.expect("count_devices");

    db::common_channel::get_by_id(&pool, 1).await.expect("common_channel::get_by_id");
    db::common_channel::get_channels_for_map(&pool, None, None, None)
        .await
        .expect("get_channels_for_map");
    db::device::list_common_channels_paged(&pool, 1, 10, None, None, None)
        .await
        .expect("list_common_channels_paged");
    db::device::list_channels_paged(&pool, "34020000001320000001", 1, 10)
        .await
        .expect("list_channels_paged");
    db::device::get_channel_by_device_and_channel_id(
        &pool,
        "34020000001320000001",
        "34020000001310000001",
    )
    .await
    .expect("get_channel_by_device_and_channel_id");
}

/// 云录像 / 告警 / 位置：列清单同样必须完整
#[tokio::test]
async fn smoke_record_alarm_position_reads() {
    let pool = sqlite_pool_with_schema().await;
    seed(&pool).await;

    db::cloud_record::get_by_id(&pool, 1).await.expect("cloud_record::get_by_id");
    db::cloud_record::list_paged(&pool, None, None, None, None, None, 1, 10)
        .await
        .expect("cloud_record::list_paged");
    db::cloud_record::find_latest_by_stream_like(&pool, "34020000001320000001")
        .await
        .expect("find_latest_by_stream_like");

    db::alarm::get_alarm_by_id(&pool, 1).await.expect("alarm::get_alarm_by_id");

    db::mobile_position::get_latest_position(&pool, "34020000001320000001", None)
        .await
        .expect("get_latest_position");
}

/// 用户 / 推流代理：同样按真实行解码
#[tokio::test]
async fn smoke_user_and_stream_reads() {
    let pool = sqlite_pool_with_schema().await;
    seed(&pool).await;

    db::user::find_by_username(&pool, "admin").await.expect("find_by_username");
    db::user::find_by_id(&pool, 1).await.expect("find_by_id");
    db::user::get_users_paged(&pool, 1, 10).await.expect("get_users_paged");
    db::user::get_all_users(&pool).await.expect("get_all_users");

    db::stream_proxy::get_by_id(&pool, 1).await.expect("stream_proxy::get_by_id");
    db::stream_proxy::get_all_enabled_proxies(&pool)
        .await
        .expect("get_all_enabled_proxies");

    db::stream_push::get_by_id(&pool, 1).await.expect("stream_push::get_by_id");
}

/// 其余业务域：同样必须能真实解码一行
#[tokio::test]
async fn smoke_platform_plan_apikey_reads() {
    let pool = sqlite_pool_with_schema().await;
    let stmts: &[&str] = &[
        "INSERT INTO gb_platform (id, name, server_gb_id, device_gb_id, enable) \
         VALUES (1, 'up', '34020000002000000001', '34020000002000000001', 1)",
        "INSERT INTO gb_platform_channel (id, platform_id, device_channel_id) VALUES (1, 1, 1)",
        "INSERT INTO gb_platform_group (id, platform_id, group_id) VALUES (1, 1, 1)",
        "INSERT INTO gb_platform_region (id, platform_id, region_id) VALUES (1, 1, 1)",
        "INSERT INTO gb_record_plan (id, name, snap, create_time, update_time) \
         VALUES (1, 'plan-1', 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        // start/stop 在 schema 中是 INTEGER（不是时间字符串）
        "INSERT INTO gb_record_plan_item (id, plan_id, start, stop, week_day, create_time, update_time) \
         VALUES (1, 1, 0, 86400, 1, '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        "INSERT INTO gb_user_api_key (id, user_id, app, api_key, enable, create_time, update_time) \
         VALUES (1, 1, 'web', 'key-1', 1, '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        "INSERT INTO gb_common_region (device_id, name, create_time, update_time) \
         VALUES ('r-1', '区域1', '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        "INSERT INTO gb_common_group (device_id, name, business_group, create_time, update_time) \
         VALUES ('g-1', '分组1', 'bg-1', '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
    ];
    for sql in stmts {
        sqlx::query(sql)
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("seed failed: {} | {}", e, sql));
    }

    db::platform::list_platforms(&pool).await.expect("platform::list_platforms");
    db::platform::get_by_id(&pool, 1).await.expect("platform::get_by_id");
    db::platform::get_all_enabled_platforms(&pool)
        .await
        .expect("get_all_enabled_platforms");

    db::platform_channel::get_by_id(&pool, 1).await.expect("platform_channel::get_by_id");
    db::platform_group::list_by_platform(&pool, 1)
        .await
        .expect("platform_group::list_by_platform");
    db::platform_region::list_by_platform(&pool, 1)
        .await
        .expect("platform_region::list_by_platform");

    db::record_plan::get_by_id(&pool, 1).await.expect("record_plan::get_by_id");
    db::record_plan::list_items(&pool, 1).await.expect("record_plan::list_items");

    db::user_api_key::get_by_id(&pool, 1).await.expect("user_api_key::get_by_id");
    db::user_api_key::list_by_user_id(&pool, 1)
        .await
        .expect("user_api_key::list_by_user_id");

    db::region::list_all(&pool).await.expect("region::list_all");
    db::region::get_by_id(&pool, 1).await.expect("region::get_by_id");
    db::group::list_all(&pool).await.expect("group::list_all");
    db::group::get_by_id(&pool, 1).await.expect("group::get_by_id");

    db::role::list_all(&pool).await.expect("role::list_all");
    db::role::get_by_id(&pool, 1).await.expect("role::get_by_id");
}

/// JT1078 与位置历史 / 审计日志
#[tokio::test]
async fn smoke_jt1078_and_history_reads() {
    let pool = sqlite_pool_with_schema().await;
    let stmts: &[&str] = &[
        "INSERT INTO gb_jt_terminal (id, phone_number, terminal_id, status, create_time, update_time) \
         VALUES (1, '13800000001', 'T-1', 1, '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        "INSERT INTO gb_jt_channel (id, terminal_db_id, channel_id, create_time, update_time) \
         VALUES (1, 1, '1', '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
    ];
    for sql in stmts {
        sqlx::query(sql)
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("seed failed: {} | {}", e, sql));
    }

    db::jt1078::list_terminals_paged(&pool, 1, 10, None, None)
        .await
        .expect("list_terminals_paged");
    db::jt1078::get_terminal_by_phone(&pool, "13800000001")
        .await
        .expect("get_terminal_by_phone");
    db::jt1078::get_terminal_by_id(&pool, 1)
        .await
        .expect("get_terminal_by_id");
    db::jt1078::get_channel_by_id(&pool, 1)
        .await
        .expect("get_channel_by_id");
    db::jt1078::list_channels_by_terminal(&pool, 1)
        .await
        .expect("list_channels_by_terminal");
    db::jt1078::get_online_terminals(&pool)
        .await
        .expect("get_online_terminals");

    db::position_history::ensure_table(&pool)
        .await
        .expect("position_history::ensure_table");
    db::position_history::list_by_device_and_time(&pool, "dev", None, None)
        .await
        .expect("list_by_device_and_time");

    db::audit_log::ensure_table(&pool).await.expect("audit_log::ensure_table");
    db::audit_log::list_paged(&pool, None, None, None, None, 1, 10)
        .await
        .expect("audit_log::list_paged");
}

/// 回归保护：这些表此前**只存在于 init-sqlite 的建表脚本**里，而该脚本仅在
/// 「gb_device 不存在」时执行 —— 于是已有旧库升级后始终缺表，对应端点报
/// "no such table"。同时 PostgreSQL / MySQL 的 schema 也缺其中 4 张。
///
/// 本测试逐一确认它们**存在且可写**。
#[tokio::test]
async fn smoke_previously_missing_tables_exist_and_accept_writes() {
    let pool = sqlite_pool_with_schema().await;
    let now = "2026-01-01 00:00:00";

    // 4 张 JT1078 区域/路线表
    sqlx::query("INSERT INTO gb_jt_area_circle (phone_number, label, center_lat, center_lon, radius_m, create_time, update_time) VALUES ('13800000001','c',32.0,118.0,100,?,?)")
        .bind(now).bind(now).execute(&pool).await.expect("gb_jt_area_circle 应存在且可写");
    sqlx::query("INSERT INTO gb_jt_area_polygon (phone_number, label, points_json, create_time, update_time) VALUES ('13800000001','p','[]',?,?)")
        .bind(now).bind(now).execute(&pool).await.expect("gb_jt_area_polygon 应存在且可写");
    sqlx::query("INSERT INTO gb_jt_area_rectangle (phone_number, label, left_top_lat, left_top_lon, right_bottom_lat, right_bottom_lon, create_time, update_time) VALUES ('13800000001','r',33.0,119.0,32.0,118.0,?,?)")
        .bind(now).bind(now).execute(&pool).await.expect("gb_jt_area_rectangle 应存在且可写");
    sqlx::query("INSERT INTO gb_jt_route (phone_number, label, waypoints_json, create_time, update_time) VALUES ('13800000001','w','[]',?,?)")
        .bind(now).bind(now).execute(&pool).await.expect("gb_jt_route 应存在且可写");

    // 平台目录表（catalog_add/edit 一直在写它）
    sqlx::query("INSERT INTO gb_platform_catalog (name, parent, civil_code, business_group, platform_id, create_time, update_time) VALUES ('n','p','340200','g',1,?,?)")
        .bind(now).bind(now).execute(&pool).await.expect("gb_platform_catalog 应存在且可写");
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gb_platform_catalog")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1);

    // 更新路径（catalog_edit）同样要能命中
    let affected = sqlx::query("UPDATE gb_platform_catalog SET name = COALESCE(?, name), update_time = ? WHERE id = 1")
        .bind(Some("renamed")).bind(now).execute(&pool).await.unwrap().rows_affected();
    assert_eq!(affected, 1, "目录更新应命中 1 行");
}
