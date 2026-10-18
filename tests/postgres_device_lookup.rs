#![cfg(feature = "postgres")]

/// Run against an isolated database initialized from init-postgresql-2.7.4.sql.
#[tokio::test]
#[ignore = "requires TEST_POSTGRES_URL and an isolated PostgreSQL database"]
async fn registered_device_can_be_read_with_channel_count() {
    let url = std::env::var("TEST_POSTGRES_URL").expect("TEST_POSTGRES_URL is required");
    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("connect PostgreSQL");
    let device_id = "34020000001320009999";
    sqlx::query("DELETE FROM gb_device WHERE device_id = $1")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("clear fixture");
    sqlx::query("INSERT INTO gb_device (device_id, on_line) VALUES ($1, true)")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("insert fixture");

    let result = gbserver::db::device::get_device_by_device_id(&pool, device_id).await;
    sqlx::query("DELETE FROM gb_device WHERE device_id = $1")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("clean fixture");
    let device = result
        .expect("registered device must be decodable")
        .expect("device exists");
    assert_eq!(device.on_line, Some(true));
    assert_eq!(device.channel_count, Some(0));
}

#[tokio::test]
#[ignore = "requires TEST_POSTGRES_URL and an isolated PostgreSQL database"]
async fn catalog_channels_satisfy_required_source_columns() {
    let url = std::env::var("TEST_POSTGRES_URL").expect("TEST_POSTGRES_URL is required");
    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("connect PostgreSQL");
    let device_id = "34020000001320009998";
    sqlx::query("DELETE FROM gb_device_channel WHERE device_id = $1")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("clear channels");
    sqlx::query("DELETE FROM gb_device WHERE device_id = $1")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("clear device");
    let parent_id: i32 = sqlx::query_scalar(
        "INSERT INTO gb_device (device_id, on_line) VALUES ($1, true) RETURNING id",
    )
    .bind(device_id)
    .fetch_one(&pool)
    .await
    .expect("seed device");

    let single = gbserver::db::device::upsert_channel_from_catalog(
        &pool,
        device_id,
        "34020000001310009998",
        "Camera",
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
    let batch = if single.is_ok() {
        gbserver::db::device::batch_upsert_channels(
            &pool,
            device_id,
            &[gbserver::db::device::ChannelInsertData {
                name: "Camera 2".into(),
                gb_device_id: "34020000001310009997".into(),
                status: "ON".into(),
                longitude: None,
                latitude: None,
                has_audio: Some(false),
                channel_type: None,
            }],
        )
        .await
    } else {
        Ok(0)
    };
    let insert = gbserver::db::device::batch_insert_channels(
        &pool,
        device_id,
        &[gbserver::db::device::ChannelInsertData {
            name: "Camera 3".into(),
            gb_device_id: "34020000001310009996".into(),
            status: "ON".into(),
            longitude: None,
            latitude: None,
            has_audio: Some(false),
            channel_type: None,
        }],
    )
    .await;
    let source_columns: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT data_type, data_device_id FROM gb_device_channel WHERE device_id = $1 ORDER BY gb_device_id",
    )
    .bind(device_id)
    .fetch_all(&pool)
    .await
    .expect("read source columns");

    sqlx::query("DELETE FROM gb_device_channel WHERE device_id = $1")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("clean channels");
    sqlx::query("DELETE FROM gb_device WHERE device_id = $1")
        .bind(device_id)
        .execute(&pool)
        .await
        .expect("clean device");

    assert_eq!(single.expect("single catalog upsert"), true);
    assert_eq!(batch.expect("batch catalog upsert"), 1);
    assert_eq!(insert.expect("batch insert"), 1);
    assert_eq!(source_columns, vec![(0, parent_id); 3]);
}
