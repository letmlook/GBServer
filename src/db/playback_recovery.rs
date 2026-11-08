//! Persistent playback recovery journal. The call sites must journal before SIP side effects.
use super::Pool;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct RecoveryRecord {
    pub stream_id: String,
    pub call_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub ssrc: String,
    pub peer_addr: String,
    pub transport: String,
    pub local_tag: Option<String>,
    pub remote_tag: Option<String>,
    pub media_server_id: String,
    pub app: String,
    pub state: String,
    pub last_cseq: i64,
}

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS gb_playback_recovery (\
    stream_id VARCHAR(128) PRIMARY KEY, call_id VARCHAR(128) NOT NULL UNIQUE,\
    device_id VARCHAR(128) NOT NULL, channel_id VARCHAR(128) NOT NULL,\
    ssrc VARCHAR(10) NOT NULL, peer_addr VARCHAR(128) NOT NULL, transport VARCHAR(16) NOT NULL,\
    local_tag VARCHAR(128), remote_tag VARCHAR(128), media_server_id VARCHAR(128) NOT NULL,\
    app VARCHAR(128) NOT NULL, state VARCHAR(32) NOT NULL, last_cseq BIGINT NOT NULL)";

pub async fn ensure_table(pool: &Pool) -> sqlx::Result<()> {
    crate::dispatch_exec!(pool, |p| { sqlx::query(CREATE_TABLE).execute(p) })?;
    Ok(())
}

pub async fn insert(pool: &Pool, record: &RecoveryRecord) -> sqlx::Result<()> {
    let sql = pool.sql("INSERT INTO gb_playback_recovery (stream_id, call_id, device_id, channel_id, ssrc, peer_addr, transport, local_tag, remote_tag, media_server_id, app, state, last_cseq) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)");
    crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(&record.stream_id)
            .bind(&record.call_id)
            .bind(&record.device_id)
            .bind(&record.channel_id)
            .bind(&record.ssrc)
            .bind(&record.peer_addr)
            .bind(&record.transport)
            .bind(&record.local_tag)
            .bind(&record.remote_tag)
            .bind(&record.media_server_id)
            .bind(&record.app)
            .bind(&record.state)
            .bind(record.last_cseq)
            .execute(p)
    })?;
    Ok(())
}

pub async fn get(pool: &Pool, stream_id: &str) -> sqlx::Result<Option<RecoveryRecord>> {
    let sql = pool.sql("SELECT stream_id, call_id, device_id, channel_id, ssrc, peer_addr, transport, local_tag, remote_tag, media_server_id, app, state, last_cseq FROM gb_playback_recovery WHERE stream_id = ?");
    crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, RecoveryRecord>(&sql)
            .bind(stream_id)
            .fetch_optional(p)
    })
}

/// CAS reservation is committed by the database before the caller can send a request.
pub async fn reserve_cseq(pool: &Pool, stream_id: &str) -> sqlx::Result<Option<i64>> {
    loop {
        let Some(record) = get(pool, stream_id).await? else {
            return Ok(None);
        };
        if !(0..2_147_483_647).contains(&record.last_cseq) {
            return Ok(None);
        }
        if !matches!(record.state.as_str(), "active" | "cleanup_pending")
            || !record
                .local_tag
                .as_deref()
                .is_some_and(|tag| !tag.is_empty())
            || !record
                .remote_tag
                .as_deref()
                .is_some_and(|tag| !tag.is_empty())
        {
            return Ok(None);
        }
        let next = record.last_cseq + 1;
        // Repeat the eligibility checks in the CAS: a state change or dialog
        // invalidation can occur after the preceding read.
        let sql = pool.sql("UPDATE gb_playback_recovery SET last_cseq = ? WHERE stream_id = ? AND last_cseq = ? AND state IN ('active', 'cleanup_pending') AND local_tag IS NOT NULL AND LENGTH(local_tag) > 0 AND remote_tag IS NOT NULL AND LENGTH(remote_tag) > 0");
        let result = crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
                .bind(next)
                .bind(stream_id)
                .bind(record.last_cseq)
                .execute(p)
        })?;
        if result.rows_affected == 1 {
            return Ok(Some(next));
        }
    }
}

pub async fn delete(pool: &Pool, stream_id: &str) -> sqlx::Result<()> {
    let sql = pool.sql("DELETE FROM gb_playback_recovery WHERE stream_id = ?");
    crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(stream_id).execute(p) })?;
    Ok(())
}
