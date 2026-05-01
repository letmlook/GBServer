use redis::aio::ConnectionManager;
use redis::AsyncCommands;

/// Redis 缓存操作封装，所有函数在 Redis 不可用时 graceful fallback

const KEY_PREFIX: &str = "wvp:";

fn device_key(device_id: &str) -> String {
    format!("{}device:online:{}", KEY_PREFIX, device_id)
}

fn stream_key(key: &str) -> String {
    format!("{}stream:{}", KEY_PREFIX, key)
}

fn media_server_streams_key(server_id: &str) -> String {
    format!("{}ms:streams:{}", KEY_PREFIX, server_id)
}

fn recording_key(device_id: &str, channel_id: &str) -> String {
    format!("{}recording:{}:{}", KEY_PREFIX, device_id, channel_id)
}

// --------------- 设备在线状态 ---------------

pub async fn set_device_online(
    redis: &ConnectionManager,
    device_id: &str,
    online: bool,
    ttl_secs: u64,
) {
    let mut conn = redis.clone();
    let key = device_key(device_id);
    let val = if online { "1" } else { "0" };
    let _: Result<(), _> = conn.set_ex(&key, val, ttl_secs).await;
}

pub async fn get_device_online(redis: &ConnectionManager, device_id: &str) -> Option<bool> {
    let mut conn = redis.clone();
    let key = device_key(device_id);
    let val: Option<String> = conn.get(&key).await.ok()?;
    val.map(|v| v == "1")
}

// --------------- 流信息缓存 ---------------

pub async fn set_stream_info(redis: &ConnectionManager, key: &str, info_json: &str, ttl_secs: u64) {
    let mut conn = redis.clone();
    let k = stream_key(key);
    let _: Result<(), _> = conn.set_ex(&k, info_json, ttl_secs).await;
}

pub async fn get_stream_info(redis: &ConnectionManager, key: &str) -> Option<String> {
    let mut conn = redis.clone();
    let k = stream_key(key);
    conn.get(&k).await.ok()?
}

// --------------- 媒体服务器流计数 (负载均衡) ---------------

pub async fn incr_media_server_streams(redis: &ConnectionManager, server_id: &str) -> i64 {
    let mut conn = redis.clone();
    let key = media_server_streams_key(server_id);
    conn.incr(&key, 1i64).await.unwrap_or(0)
}

pub async fn decr_media_server_streams(redis: &ConnectionManager, server_id: &str) -> i64 {
    let mut conn = redis.clone();
    let key = media_server_streams_key(server_id);
    let val: i64 = conn.decr(&key, 1i64).await.unwrap_or(0);
    if val < 0 {
        let _: Result<(), _> = conn.set(&key, 0i64).await;
        return 0;
    }
    val
}

pub async fn get_media_server_stream_count(redis: &ConnectionManager, server_id: &str) -> i64 {
    let mut conn = redis.clone();
    let key = media_server_streams_key(server_id);
    conn.get(&key).await.unwrap_or(0)
}

/// 重置某个节点的流计数（用于 ZLM 重启等场景）
pub async fn reset_media_server_streams(redis: &ConnectionManager, server_id: &str, count: i64) {
    let mut conn = redis.clone();
    let key = media_server_streams_key(server_id);
    let _: Result<(), _> = conn.set(&key, count).await;
}

/// Alias for reset_media_server_streams
pub async fn set_media_server_streams(redis: &ConnectionManager, server_id: &str, count: i64) {
    reset_media_server_streams(redis, server_id, count).await;
}

// --------------- 录像状态 ---------------

pub async fn set_recording_state(
    redis: &ConnectionManager,
    device_id: &str,
    channel_id: &str,
    cmd: &str,
) {
    let mut conn = redis.clone();
    let key = recording_key(device_id, channel_id);
    // 录像状态 TTL 24h，防止泄漏
    let _: Result<(), _> = conn.set_ex(&key, cmd, 86400).await;
}

pub async fn get_recording_state(
    redis: &ConnectionManager,
    device_id: &str,
    channel_id: &str,
) -> Option<String> {
    let mut conn = redis.clone();
    let key = recording_key(device_id, channel_id);
    conn.get(&key).await.ok()?
}

pub async fn del_recording_state(redis: &ConnectionManager, device_id: &str, channel_id: &str) {
    let mut conn = redis.clone();
    let key = recording_key(device_id, channel_id);
    let _: Result<(), _> = conn.del(&key).await;
}
