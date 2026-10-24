#![cfg(feature = "mysql")]

/// Run against an isolated MySQL database initialized from init-mysql-2.7.4.sql.
#[tokio::test]
#[ignore = "requires TEST_MYSQL_URL and an isolated MySQL database"]
async fn catalog_writes_satisfy_required_channel_columns() {
    let url = std::env::var("TEST_MYSQL_URL").expect("TEST_MYSQL_URL is required");
    let pool = sqlx::MySqlPool::connect(&url).await.expect("connect MySQL");
    // `db::device::*` 现在接受运行期多态的 `db::Pool`
    // （原生 MySqlPool 仍用于直连 sqlx 查询）。
    let db_pool = gbserver::db::Pool::MySql(pool.clone());
    let device_id = "34020000001320009996";
    sqlx::query("DELETE FROM gb_device_channel WHERE device_id = ?")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("clear channels");
    sqlx::query("DELETE FROM gb_device WHERE device_id = ?")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("clear device");
    sqlx::query("INSERT INTO gb_device (device_id, on_line) VALUES (?, 1)")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("seed device");
    let parent_id: i32 = sqlx::query_scalar("SELECT id FROM gb_device WHERE device_id = ?")
        .bind(device_id)
        .fetch_one(&pool)
        .await
        .expect("parent id");

    let single = gbserver::db::device::upsert_channel_from_catalog(
        &db_pool,
        device_id,
        "34020000001310009996",
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
    .await;
    let channel = |id: &str| gbserver::db::device::ChannelInsertData {
        name: id.into(),
        gb_device_id: id.into(),
        status: "ON".into(),
        longitude: None,
        latitude: None,
        has_audio: Some(false),
        channel_type: None,
    };
    let batch = gbserver::db::device::batch_upsert_channels(
        &db_pool,
        device_id,
        &[channel("34020000001310009995")],
    )
    .await;
    let insert = gbserver::db::device::batch_insert_channels(
        &db_pool,
        device_id,
        &[channel("34020000001310009994")],
    )
    .await;
    let rows: Vec<(i32, i32, i32)> = sqlx::query_as(
        "SELECT data_type, data_device_id, channel_type FROM gb_device_channel WHERE device_id = ? ORDER BY gb_device_id",
    )
    .bind(device_id)
    .fetch_all(&pool)
    .await
    .expect("read channels");
    sqlx::query("DELETE FROM gb_device_channel WHERE device_id = ?")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("clean channels");
    sqlx::query("DELETE FROM gb_device WHERE device_id = ?")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("clean device");

    assert!(single.expect("single catalog upsert"));
    assert_eq!(batch.expect("batch catalog upsert"), 1);
    assert_eq!(insert.expect("batch insert"), 1);
    assert_eq!(rows, vec![(0, parent_id, 0); 3]);
}
