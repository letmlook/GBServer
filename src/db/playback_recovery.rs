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

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct DialogAddress {
    pub stream_id: String,
    pub request_uri: String,
    pub from_header: String,
    pub to_header: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ObservedRoute {
    pub stream_id: String,
    pub peer_addr: String,
    pub transport: String,
}

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS gb_playback_recovery (\
    stream_id VARCHAR(128) PRIMARY KEY, call_id VARCHAR(128) NOT NULL UNIQUE,\
    device_id VARCHAR(128) NOT NULL, channel_id VARCHAR(128) NOT NULL,\
    ssrc VARCHAR(10) NOT NULL, peer_addr VARCHAR(128) NOT NULL, transport VARCHAR(16) NOT NULL,\
    local_tag VARCHAR(128), remote_tag VARCHAR(128), media_server_id VARCHAR(128) NOT NULL,\
    app VARCHAR(128) NOT NULL, state VARCHAR(32) NOT NULL, last_cseq BIGINT NOT NULL)";

pub async fn ensure_table(pool: &Pool) -> sqlx::Result<()> {
    crate::dispatch_exec!(pool, |p| { sqlx::query(CREATE_TABLE).execute(p) })?;
    crate::dispatch_exec!(pool, |p| {
        sqlx::query("CREATE TABLE IF NOT EXISTS gb_playback_dialog_address (stream_id VARCHAR(128) PRIMARY KEY, request_uri TEXT NOT NULL, from_header TEXT NOT NULL, to_header TEXT NOT NULL)").execute(p)
    })?;
    crate::dispatch_exec!(pool, |p| {
        sqlx::query("CREATE TABLE IF NOT EXISTS gb_playback_observed_route (stream_id VARCHAR(128) PRIMARY KEY, peer_addr VARCHAR(128) NOT NULL, transport VARCHAR(16) NOT NULL)").execute(p)
    })?;
    crate::dispatch_exec!(pool, |p| {
        sqlx::query("CREATE TABLE IF NOT EXISTS gb_playback_safe_routing (stream_id VARCHAR(128) PRIMARY KEY, contact_uri TEXT NOT NULL, record_routes_json TEXT NOT NULL)").execute(p)
    })?;
    Ok(())
}

fn validate_safe_routing(routing: &crate::sip::gb28181::recovery_routing::SafeRouting) -> bool {
    let input = crate::sip::gb28181::invite_session::SipRoutingHeaders {
        contacts: vec![format!("<{}>", routing.contact_uri)],
        record_routes: routing
            .record_route_uris
            .iter()
            .map(|uri| format!("<{}>", uri))
            .collect(),
    };
    crate::sip::gb28181::recovery_routing::safe_routing(&input).as_ref() == Some(routing)
}

pub async fn insert_safe_routing(
    pool: &Pool,
    stream_id: &str,
    routing: &crate::sip::gb28181::recovery_routing::SafeRouting,
) -> sqlx::Result<()> {
    if !validate_safe_routing(routing) {
        return Err(sqlx::Error::Protocol("invalid recovery routing".into()));
    }
    let routes = serde_json::to_string(&routing.record_route_uris)
        .map_err(|_| sqlx::Error::Protocol("invalid recovery routing".into()))?;
    let sql = pool.sql("INSERT INTO gb_playback_safe_routing (stream_id, contact_uri, record_routes_json) VALUES (?, ?, ?)");
    crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(stream_id)
            .bind(&routing.contact_uri)
            .bind(&routes)
            .execute(p)
    })?;
    Ok(())
}

pub async fn get_safe_routing(
    pool: &Pool,
    stream_id: &str,
) -> sqlx::Result<Option<crate::sip::gb28181::recovery_routing::SafeRouting>> {
    let sql = pool.sql(
        "SELECT contact_uri, record_routes_json FROM gb_playback_safe_routing WHERE stream_id = ?",
    );
    let row: Option<(String, String)> = crate::dispatch!(pool, |p| {
        sqlx::query_as(&sql).bind(stream_id).fetch_optional(p)
    })?;
    let Some((contact_uri, routes)) = row else {
        return Ok(None);
    };
    let record_route_uris = serde_json::from_str(&routes)
        .map_err(|_| sqlx::Error::Protocol("invalid recovery routing".into()))?;
    let routing = crate::sip::gb28181::recovery_routing::SafeRouting {
        contact_uri,
        record_route_uris,
    };
    if !validate_safe_routing(&routing) {
        return Err(sqlx::Error::Protocol("invalid recovery routing".into()));
    }
    Ok(Some(routing))
}

pub async fn insert_observed_route(pool: &Pool, route: &ObservedRoute) -> sqlx::Result<()> {
    let sql = pool.sql(
        "INSERT INTO gb_playback_observed_route (stream_id, peer_addr, transport) VALUES (?, ?, ?)",
    );
    crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(&route.stream_id)
            .bind(&route.peer_addr)
            .bind(&route.transport)
            .execute(p)
    })?;
    Ok(())
}

pub async fn get_observed_route(
    pool: &Pool,
    stream_id: &str,
) -> sqlx::Result<Option<ObservedRoute>> {
    let sql = pool.sql("SELECT stream_id, peer_addr, transport FROM gb_playback_observed_route WHERE stream_id = ?");
    crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, ObservedRoute>(&sql)
            .bind(stream_id)
            .fetch_optional(p)
    })
}

pub async fn insert_dialog_address(pool: &Pool, address: &DialogAddress) -> sqlx::Result<()> {
    if !crate::sip::gb28181::recovery_routing::safe_dialog_address(
        &address.request_uri,
        &address.from_header,
        &address.to_header,
    ) {
        return Err(sqlx::Error::Protocol(
            "invalid recovery dialog address".into(),
        ));
    }
    let sql = pool.sql("INSERT INTO gb_playback_dialog_address (stream_id, request_uri, from_header, to_header) VALUES (?, ?, ?, ?)");
    crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(&address.stream_id)
            .bind(&address.request_uri)
            .bind(&address.from_header)
            .bind(&address.to_header)
            .execute(p)
    })?;
    Ok(())
}

pub async fn get_dialog_address(
    pool: &Pool,
    stream_id: &str,
) -> sqlx::Result<Option<DialogAddress>> {
    let sql = pool.sql("SELECT stream_id, request_uri, from_header, to_header FROM gb_playback_dialog_address WHERE stream_id = ?");
    let result: Option<DialogAddress> = crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, DialogAddress>(&sql)
            .bind(stream_id)
            .fetch_optional(p)
    })?;
    if result.as_ref().is_some_and(|address| {
        !crate::sip::gb28181::recovery_routing::safe_dialog_address(
            &address.request_uri,
            &address.from_header,
            &address.to_header,
        )
    }) {
        return Err(sqlx::Error::Protocol(
            "invalid recovery dialog address".into(),
        ));
    }
    Ok(result)
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

/// Every retained entry may still own a remote stream, including entries whose
/// local cleanup completed without a confirmed device response.
pub async fn list_retained(pool: &Pool) -> sqlx::Result<Vec<RecoveryRecord>> {
    let sql = "SELECT stream_id, call_id, device_id, channel_id, ssrc, peer_addr, transport, local_tag, remote_tag, media_server_id, app, state, last_cseq FROM gb_playback_recovery ORDER BY stream_id";
    crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, RecoveryRecord>(sql).fetch_all(p)
    })
}

/// Complete a previously journaled INVITE exactly once; late responses cannot
/// replace the established tag or revive an entry already being cleaned up.
pub async fn activate_dialog(pool: &Pool, stream_id: &str, remote_tag: &str) -> sqlx::Result<bool> {
    if remote_tag.is_empty() {
        return Ok(false);
    }
    let sql = pool.sql("UPDATE gb_playback_recovery SET remote_tag = ?, state = 'active' WHERE stream_id = ? AND state = 'inviting' AND remote_tag IS NULL AND local_tag IS NOT NULL AND LENGTH(local_tag) > 0");
    let result = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(remote_tag)
            .bind(stream_id)
            .execute(p)
    })?;
    Ok(result.rows_affected == 1)
}

/// Preserve an established dialog for explicit cleanup after a local failure.
pub async fn mark_cleanup_pending(pool: &Pool, stream_id: &str) -> sqlx::Result<bool> {
    let sql = pool.sql("UPDATE gb_playback_recovery SET state = 'cleanup_pending' WHERE stream_id = ? AND state = 'active'");
    let result = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(stream_id).execute(p) })?;
    Ok(result.rows_affected == 1)
}

/// CAS reservation is committed by the database before the caller can send a request.
pub async fn reserve_cseq(pool: &Pool, stream_id: &str) -> sqlx::Result<Option<i64>> {
    reserve_cseq_at_least(pool, stream_id, None, 1).await
}

pub async fn reserve_cseq_at_least(
    pool: &Pool,
    stream_id: &str,
    expected_call_id: Option<&str>,
    minimum: i64,
) -> sqlx::Result<Option<i64>> {
    if !(1..=2_147_483_647).contains(&minimum) {
        return Ok(None);
    }
    loop {
        let Some(record) = get(pool, stream_id).await? else {
            return Ok(None);
        };
        if !(0..2_147_483_647).contains(&record.last_cseq)
            || expected_call_id.is_some_and(|id| id != record.call_id)
        {
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
        let next = (record.last_cseq + 1).max(minimum);
        // Repeat the eligibility checks in the CAS: a state change or dialog
        // invalidation can occur after the preceding read.
        let sql = pool.sql("UPDATE gb_playback_recovery SET last_cseq = ? WHERE stream_id = ? AND call_id = ? AND last_cseq = ? AND state IN ('active', 'cleanup_pending') AND local_tag IS NOT NULL AND LENGTH(local_tag) > 0 AND remote_tag IS NOT NULL AND LENGTH(remote_tag) > 0");
        let result = crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
                .bind(next)
                .bind(stream_id)
                .bind(&record.call_id)
                .bind(record.last_cseq)
                .execute(p)
        })?;
        if result.rows_affected == 1 {
            return Ok(Some(next));
        }
    }
}

pub async fn delete(pool: &Pool, stream_id: &str) -> sqlx::Result<()> {
    let routing_sql = pool.sql("DELETE FROM gb_playback_safe_routing WHERE stream_id = ?");
    let route_sql = pool.sql("DELETE FROM gb_playback_observed_route WHERE stream_id = ?");
    let address_sql = pool.sql("DELETE FROM gb_playback_dialog_address WHERE stream_id = ?");
    let record_sql = pool.sql("DELETE FROM gb_playback_recovery WHERE stream_id = ?");
    crate::dispatch!(pool, |p| {
        async {
            let mut tx = p.begin().await?;
            sqlx::query(&routing_sql)
                .bind(stream_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query(&route_sql)
                .bind(stream_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query(&address_sql)
                .bind(stream_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query(&record_sql)
                .bind(stream_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await
        }
    })
}
