#![cfg(feature = "sqlite")]

#[tokio::test]
async fn catalog_writes_use_production_sqlite_schema() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("open SQLite");
    // `db::device::*` 现在接受运行期多态的 `db::Pool`
    // （原生 SqlitePool 仍用于直连 sqlx 查询）。
    let db_pool = gbserver::db::Pool::Sqlite(pool.clone());
    let schema = include_str!("../database/init-sqlite-2.7.4.sql");
    let cleaned = schema
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    for statement in cleaned.split(';') {
        let statement = statement.trim();
        if statement.starts_with("CREATE") || statement.starts_with("INSERT") {
            sqlx::query(statement)
                .execute(&pool)
                .await
                .expect("initialize production schema");
        }
    }
    let device_id = "34020000001320009993";
    sqlx::query("INSERT INTO gb_device (device_id, on_line) VALUES (?, 1)")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("seed device");
    let parent_id: i64 = sqlx::query_scalar("SELECT id FROM gb_device WHERE device_id = ?")
        .bind(device_id)
        .fetch_one(&pool)
        .await
        .expect("parent id");

    assert!(gbserver::db::device::upsert_channel_from_catalog(
        &db_pool,
        device_id,
        "34020000001310009993",
        "Camera 1",
        None,
        None,
        None,
        None,
        None,
        None,
        true,
        None,
        None,
        None,
        Some(false),
        None,
    )
    .await
    .expect("single catalog upsert"));
    let channel = |id: &str| gbserver::db::device::ChannelInsertData {
        name: id.into(),
        gb_device_id: id.into(),
        status: "ON".into(),
        longitude: None,
        latitude: None,
        has_audio: Some(false),
        channel_type: None,
    };
    assert_eq!(
        gbserver::db::device::batch_upsert_channels(
            &db_pool,
            device_id,
            &[channel("34020000001310009992")],
        )
        .await
        .expect("batch catalog upsert"),
        1
    );
    assert_eq!(
        gbserver::db::device::batch_insert_channels(
            &db_pool,
            device_id,
            &[channel("34020000001310009991")],
        )
        .await
        .expect("batch insert"),
        1
    );
    let rows: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT data_type, data_device_id, channel_type FROM gb_device_channel WHERE device_id = ? ORDER BY gb_device_id",
    )
    .bind(device_id)
    .fetch_all(&pool)
    .await
    .expect("read channels");
    assert_eq!(rows, vec![(0, parent_id, 0); 3]);
}
