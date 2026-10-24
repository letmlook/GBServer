#![cfg(feature = "postgres")]

/// Run with TEST_POSTGRES_URL pointing at an isolated database initialized from
/// database/init-postgresql-2.7.4.sql. This test inserts a row into that database.
#[tokio::test]
#[ignore = "requires TEST_POSTGRES_URL and an isolated initialized PostgreSQL database"]
async fn update_custom_fields_roundtrip() {
    let url = std::env::var("TEST_POSTGRES_URL").expect("TEST_POSTGRES_URL is required");
    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("connect PostgreSQL");
    // `db::platform_channel::*` 现在接受运行期多态的 `db::Pool`
    // （原生 PgPool 仍用于直连 sqlx 查询）。
    let db_pool = gbserver::db::Pool::Postgres(pool.clone());
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO gb_platform_channel (platform_id, device_channel_id) VALUES (NULL, NULL) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("seed channel");
    let id = i64::from(id);

    let changed = gbserver::db::platform_channel::update(
        &db_pool,
        id,
        Some("O'Brien"),
        Some("V2 custom address"),
    )
    .await
    .expect("update both fields");
    assert_eq!(changed, 1);
    let fields: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT custom_name, custom_address FROM gb_platform_channel WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("read updated channel");
    assert_eq!(fields.0.as_deref(), Some("O'Brien"));
    assert_eq!(fields.1.as_deref(), Some("V2 custom address"));

    assert_eq!(
        gbserver::db::platform_channel::update(&db_pool, id, None, Some("north gate"))
            .await
            .expect("update address only"),
        1,
    );
    assert_eq!(
        gbserver::db::platform_channel::update(&db_pool, id, Some("V2 name"), None)
            .await
            .expect("update name only"),
        1,
    );
    assert_eq!(
        gbserver::db::platform_channel::update(&db_pool, id, None, None)
            .await
            .expect("empty update"),
        0,
    );
    let fields: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT custom_name, custom_address FROM gb_platform_channel WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("read partial updates");
    assert_eq!(fields.0.as_deref(), Some("V2 name"));
    assert_eq!(fields.1.as_deref(), Some("north gate"));
}
