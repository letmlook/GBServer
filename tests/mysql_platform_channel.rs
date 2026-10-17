#![cfg(feature = "mysql")]

/// Run with TEST_MYSQL_URL pointing at an isolated database initialized from
/// database/init-mysql-2.7.4.sql. This test inserts a row into that database.
#[tokio::test]
#[ignore = "requires TEST_MYSQL_URL and an isolated initialized MySQL database"]
async fn update_custom_fields_roundtrip() {
    let url = std::env::var("TEST_MYSQL_URL").expect("TEST_MYSQL_URL is required");
    let pool = sqlx::MySqlPool::connect(&url).await.expect("connect MySQL");
    let id = sqlx::query(
        "INSERT INTO gb_platform_channel (platform_id, device_channel_id) VALUES (NULL, NULL)",
    )
    .execute(&pool)
    .await
    .expect("seed channel")
    .last_insert_id() as i64;

    let changed = gbserver::db::platform_channel::update(
        &pool,
        id,
        Some("V2 custom name"),
        Some("V2 custom address"),
    )
    .await
    .expect("update channel custom fields");
    assert_eq!(changed, 1);
    let fields: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT custom_name, custom_address FROM gb_platform_channel WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("read updated channel");
    assert_eq!(fields.0.as_deref(), Some("V2 custom name"));
    assert_eq!(fields.1.as_deref(), Some("V2 custom address"));

    assert_eq!(
        gbserver::db::platform_channel::update(&pool, id, Some("O'Brien"), None)
            .await
            .expect("update name only"),
        1,
    );
    assert_eq!(
        gbserver::db::platform_channel::update(&pool, id, None, Some("north gate"))
            .await
            .expect("update address only"),
        1,
    );
    assert_eq!(
        gbserver::db::platform_channel::update(&pool, id, None, None)
            .await
            .expect("empty update"),
        0,
    );
    let fields: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT custom_name, custom_address FROM gb_platform_channel WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("read partial updates");
    assert_eq!(fields.0.as_deref(), Some("O'Brien"));
    assert_eq!(fields.1.as_deref(), Some("north gate"));
}
