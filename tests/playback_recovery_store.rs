use gbserver::db::{playback_recovery as journal, Pool};

fn sample_record() -> journal::RecoveryRecord {
    let id = uuid::Uuid::new_v4().to_string();
    journal::RecoveryRecord {
        stream_id: format!("stream_{id}"),
        call_id: format!("call_{id}"),
        device_id: "device".into(),
        channel_id: "channel".into(),
        ssrc: "1200000001".into(),
        peer_addr: "127.0.0.1:5060".into(),
        transport: "udp".into(),
        local_tag: Some("local".into()),
        remote_tag: Some("remote".into()),
        media_server_id: "zlm".into(),
        app: "playback".into(),
        state: "active".into(),
        last_cseq: 1,
    }
}

async fn exercise(pool: Pool) {
    journal::ensure_table(&pool).await.unwrap();
    journal::ensure_table(&pool).await.unwrap();
    let record = sample_record();
    journal::insert(&pool, &record).await.unwrap();
    assert!(journal::list_retained(&pool)
        .await
        .unwrap()
        .contains(&record));
    assert!(journal::insert(&pool, &record).await.is_err());
    assert_eq!(
        journal::get(&pool, &record.stream_id).await.unwrap(),
        Some(record.clone())
    );
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        let stream_id = record.stream_id.clone();
        tasks.push(tokio::spawn(async move {
            journal::reserve_cseq(&pool, &stream_id)
                .await
                .unwrap()
                .unwrap()
        }));
    }
    let mut values = Vec::new();
    for task in tasks {
        values.push(task.await.unwrap());
    }
    values.sort_unstable();
    assert_eq!(values, (2..=9).collect::<Vec<_>>());
    assert_eq!(
        journal::get(&pool, &record.stream_id)
            .await
            .unwrap()
            .unwrap()
            .last_cseq,
        9
    );
    assert_eq!(journal::reserve_cseq(&pool, "missing").await.unwrap(), None);
    journal::delete(&pool, &record.stream_id).await.unwrap();
    let mut exhausted = sample_record();
    exhausted.last_cseq = 2_147_483_647;
    journal::insert(&pool, &exhausted).await.unwrap();
    assert_eq!(
        journal::reserve_cseq(&pool, &exhausted.stream_id)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        journal::get(&pool, &exhausted.stream_id)
            .await
            .unwrap()
            .unwrap()
            .last_cseq,
        exhausted.last_cseq
    );
    journal::delete(&pool, &exhausted.stream_id).await.unwrap();
    for (state, local, remote) in [
        ("inviting", Some("local"), Some("remote")),
        ("active", None, Some("remote")),
        ("active", Some("local"), Some("")),
        ("local_cleaned", Some("local"), Some("remote")),
    ] {
        let mut incomplete = sample_record();
        incomplete.state = state.into();
        incomplete.local_tag = local.map(str::to_string);
        incomplete.remote_tag = remote.map(str::to_string);
        journal::insert(&pool, &incomplete).await.unwrap();
        assert_eq!(
            journal::reserve_cseq(&pool, &incomplete.stream_id)
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            journal::get(&pool, &incomplete.stream_id)
                .await
                .unwrap()
                .unwrap()
                .last_cseq,
            1
        );
        journal::delete(&pool, &incomplete.stream_id).await.unwrap();
    }
    let mut pending = sample_record();
    pending.state = "cleanup_pending".into();
    journal::insert(&pool, &pending).await.unwrap();
    assert_eq!(
        journal::reserve_cseq(&pool, &pending.stream_id)
            .await
            .unwrap(),
        Some(2)
    );
    journal::delete(&pool, &pending.stream_id).await.unwrap();

    let mut dialog = sample_record();
    dialog.state = "inviting".into();
    dialog.remote_tag = None;
    journal::insert(&pool, &dialog).await.unwrap();
    assert!(!journal::activate_dialog(&pool, &dialog.stream_id, "")
        .await
        .unwrap());
    assert!(
        journal::activate_dialog(&pool, &dialog.stream_id, "device-tag")
            .await
            .unwrap()
    );
    assert!(
        !journal::activate_dialog(&pool, &dialog.stream_id, "different-tag")
            .await
            .unwrap()
    );
    assert_eq!(
        journal::get(&pool, &dialog.stream_id)
            .await
            .unwrap()
            .unwrap()
            .remote_tag
            .as_deref(),
        Some("device-tag")
    );
    assert!(journal::mark_cleanup_pending(&pool, &dialog.stream_id)
        .await
        .unwrap());
    assert!(
        !journal::activate_dialog(&pool, &dialog.stream_id, "late-tag")
            .await
            .unwrap()
    );
    assert_eq!(
        journal::reserve_cseq(&pool, &dialog.stream_id)
            .await
            .unwrap(),
        Some(2)
    );
    journal::delete(&pool, &dialog.stream_id).await.unwrap();
}

#[tokio::test]
async fn sqlite_journal_round_trip_and_concurrent_reservation() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    exercise(Pool::Sqlite(pool)).await;
}

#[tokio::test]
#[ignore = "requires TEST_POSTGRES_URL"]
async fn postgres_journal_round_trip_and_concurrent_reservation() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_POSTGRES_URL").unwrap())
        .await
        .unwrap();
    exercise(Pool::Postgres(pool)).await;
}

#[tokio::test]
#[ignore = "requires TEST_MYSQL_URL"]
async fn mysql_journal_round_trip_and_concurrent_reservation() {
    let pool = sqlx::MySqlPool::connect(&std::env::var("TEST_MYSQL_URL").unwrap())
        .await
        .unwrap();
    exercise(Pool::MySql(pool)).await;
}

#[tokio::test]
async fn sqlite_journal_survives_connection_reopen() {
    let path = std::env::temp_dir().join(format!("gbserver-recovery-{}.db", uuid::Uuid::new_v4()));
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let db = sqlx::SqlitePool::connect_with(options.clone())
        .await
        .unwrap();
    let pool = Pool::Sqlite(db.clone());
    journal::ensure_table(&pool).await.unwrap();
    let record = sample_record();
    journal::insert(&pool, &record).await.unwrap();
    assert_eq!(
        journal::reserve_cseq(&pool, &record.stream_id)
            .await
            .unwrap(),
        Some(2)
    );
    db.close().await;
    let db = sqlx::SqlitePool::connect_with(options).await.unwrap();
    let pool = Pool::Sqlite(db.clone());
    let loaded = journal::get(&pool, &record.stream_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.last_cseq, 2);
    assert_eq!(loaded.call_id, record.call_id);
    assert_eq!(
        journal::reserve_cseq(&pool, &record.stream_id)
            .await
            .unwrap(),
        Some(3)
    );
    db.close().await;
    std::fs::remove_file(path).unwrap();
}
