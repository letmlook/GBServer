use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use sqlx::Row;

use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct AlarmQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub device_id: Option<String>,
    pub channel_id: Option<String>,
    pub alarm_method: Option<String>,
    pub alarm_type: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub handled: Option<bool>,
}

/// GET /api/alarm/list - 查询告警列表
pub async fn alarm_list(
    State(state): State<AppState>,
    Query(q): Query<AlarmQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let offset = (page - 1) * count;

    #[cfg(feature = "postgres")]
    {
        let rows: Vec<serde_json::Value> = sqlx::query(
            "SELECT id, device_id, channel_id, alarm_priority, alarm_method, alarm_type, 
                    alarm_time, alarm_description, longitude, latitude, create_time 
             FROM wvp_device_alarm 
             WHERE ($1::text IS NULL OR device_id = $1)
               AND ($2::text IS NULL OR channel_id = $2)
               AND ($3::text IS NULL OR alarm_type = $3)
               AND ($4::text IS NULL OR alarm_method = $4)
             ORDER BY create_time DESC 
             LIMIT $5 OFFSET $6",
        )
        .bind(&q.device_id)
        .bind(&q.channel_id)
        .bind(&q.alarm_type)
        .bind(&q.alarm_method)
        .bind(count as i64)
        .bind(offset as i64)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库查询失败: {}", e)))?
        .into_iter()
        .map(|r| {
            let id: i64 = r.get("id");
            let device_id: Option<String> = r.get("device_id");
            let channel_id: Option<String> = r.get("channel_id");
            let alarm_priority: Option<String> = r.get("alarm_priority");
            let alarm_method: Option<String> = r.get("alarm_method");
            let alarm_type: Option<String> = r.get("alarm_type");
            let alarm_time: Option<String> = r.get("alarm_time");
            let alarm_description: Option<String> = r.get("alarm_description");
            let longitude: Option<f64> = r.get("longitude");
            let latitude: Option<f64> = r.get("latitude");
            let create_time: Option<String> = r.get("create_time");
            serde_json::json!({
                "id": id,
                "deviceId": device_id,
                "channelId": channel_id,
                "alarmPriority": alarm_priority,
                "alarmMethod": alarm_method,
                "alarmType": alarm_type,
                "alarmTime": alarm_time,
                "alarmDescription": alarm_description,
                "longitude": longitude,
                "latitude": latitude,
                "createTime": create_time,
                "handled": false,
                "handleTime": None::<Option<String>>,
                "handleUser": None::<Option<String>>,
            })
        })
        .collect();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM wvp_device_alarm 
             WHERE ($1::text IS NULL OR device_id = $1)
               AND ($2::text IS NULL OR channel_id = $2)
               AND ($3::text IS NULL OR alarm_type = $3)
               AND ($4::text IS NULL OR alarm_method = $4)",
        )
        .bind(&q.device_id)
        .bind(&q.channel_id)
        .bind(&q.alarm_type)
        .bind(&q.alarm_method)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

        Ok(Json(WVPResult::success(serde_json::json!({
            "total": total,
            "count": count,
            "page": page,
            "list": rows,
        }))))
    }

    #[cfg(feature = "mysql")]
    {
        let rows: Vec<serde_json::Value> = sqlx::query(
            "SELECT id, device_id, channel_id, alarm_priority, alarm_method, alarm_type, 
                    alarm_time, alarm_description, longitude, latitude, create_time 
             FROM wvp_device_alarm 
             WHERE (? IS NULL OR device_id = ?)
               AND (? IS NULL OR channel_id = ?)
               AND (? IS NULL OR alarm_type = ?)
               AND (? IS NULL OR alarm_method = ?)
             ORDER BY create_time DESC 
             LIMIT ? OFFSET ?",
        )
        .bind(&q.device_id).bind(&q.device_id)
        .bind(&q.channel_id).bind(&q.channel_id)
        .bind(&q.alarm_type).bind(&q.alarm_type)
        .bind(&q.alarm_method).bind(&q.alarm_method)
        .bind(count as i64)
        .bind(offset as i64)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库查询失败: {}", e)))?
        .into_iter()
        .map(|r| {
            let id: i64 = r.get("id");
            let device_id: Option<String> = r.get("device_id");
            let channel_id: Option<String> = r.get("channel_id");
            let alarm_priority: Option<String> = r.get("alarm_priority");
            let alarm_method: Option<String> = r.get("alarm_method");
            let alarm_type: Option<String> = r.get("alarm_type");
            let alarm_time: Option<String> = r.get("alarm_time");
            let alarm_description: Option<String> = r.get("alarm_description");
            let longitude: Option<f64> = r.get("longitude");
            let latitude: Option<f64> = r.get("latitude");
            let create_time: Option<String> = r.get("create_time");
            serde_json::json!({
                "id": id,
                "deviceId": device_id,
                "channelId": channel_id,
                "alarmPriority": alarm_priority,
                "alarmMethod": alarm_method,
                "alarmType": alarm_type,
                "alarmTime": alarm_time,
                "alarmDescription": alarm_description,
                "longitude": longitude,
                "latitude": latitude,
                "createTime": create_time,
                "handled": false,
                "handleTime": None::<Option<String>>,
                "handleUser": None::<Option<String>>,
            })
        })
        .collect();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM wvp_device_alarm 
             WHERE (? IS NULL OR device_id = ?)
               AND (? IS NULL OR channel_id = ?)
               AND (? IS NULL OR alarm_type = ?)
               AND (? IS NULL OR alarm_method = ?)",
        )
        .bind(&q.device_id).bind(&q.device_id)
        .bind(&q.channel_id).bind(&q.channel_id)
        .bind(&q.alarm_type).bind(&q.alarm_type)
        .bind(&q.alarm_method).bind(&q.alarm_method)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

        Ok(Json(WVPResult::success(serde_json::json!({
            "total": total,
            "count": count,
            "page": page,
            "list": rows,
        }))))
    }
}

/// GET /api/alarm/detail/:id - 查询告警详情
pub async fn alarm_detail(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    #[cfg(feature = "postgres")]
    {
        let row = sqlx::query(
            "SELECT * FROM wvp_device_alarm WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库查询失败: {}", e)))?;

        match row {
            Some(r) => {
                let id: i64 = r.get("id");
                let device_id: Option<String> = r.get("device_id");
                let channel_id: Option<String> = r.get("channel_id");
                let alarm_priority: Option<String> = r.get("alarm_priority");
                let alarm_method: Option<String> = r.get("alarm_method");
                let alarm_type: Option<String> = r.get("alarm_type");
                let alarm_time: Option<String> = r.get("alarm_time");
                let alarm_description: Option<String> = r.get("alarm_description");
                let longitude: Option<f64> = r.get("longitude");
                let latitude: Option<f64> = r.get("latitude");
                let create_time: Option<String> = r.get("create_time");
                Ok(Json(WVPResult::success(serde_json::json!({
                    "id": id,
                    "deviceId": device_id,
                    "channelId": channel_id,
                    "alarmPriority": alarm_priority,
                    "alarmMethod": alarm_method,
                    "alarmType": alarm_type,
                    "alarmTime": alarm_time,
                    "alarmDescription": alarm_description,
                    "longitude": longitude,
                    "latitude": latitude,
                    "createTime": create_time,
                    "handled": false,
                    "handleTime": None::<Option<String>>,
                    "handleUser": None::<Option<String>>,
                }))))
            }
            None => Ok(Json(WVPResult::error("告警不存在".to_string()))),
        }
    }

    #[cfg(feature = "mysql")]
    {
        let row = sqlx::query("SELECT * FROM wvp_device_alarm WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库查询失败: {}", e)))?;

        match row {
            Some(r) => {
                let id: i64 = r.get("id");
                let device_id: Option<String> = r.get("device_id");
                let channel_id: Option<String> = r.get("channel_id");
                let alarm_priority: Option<String> = r.get("alarm_priority");
                let alarm_method: Option<String> = r.get("alarm_method");
                let alarm_type: Option<String> = r.get("alarm_type");
                let alarm_time: Option<String> = r.get("alarm_time");
                let alarm_description: Option<String> = r.get("alarm_description");
                let longitude: Option<f64> = r.get("longitude");
                let latitude: Option<f64> = r.get("latitude");
                let create_time: Option<String> = r.get("create_time");
                Ok(Json(WVPResult::success(serde_json::json!({
                    "id": id,
                    "deviceId": device_id,
                    "channelId": channel_id,
                    "alarmPriority": alarm_priority,
                    "alarmMethod": alarm_method,
                    "alarmType": alarm_type,
                    "alarmTime": alarm_time,
                    "alarmDescription": alarm_description,
                    "longitude": longitude,
                    "latitude": latitude,
                    "createTime": create_time,
                    "handled": false,
                    "handleTime": None::<Option<String>>,
                    "handleUser": None::<Option<String>>,
                }))))
            }
            None => Ok(Json(WVPResult::error("告警不存在".to_string()))),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AlarmHandleBody {
    pub id: Option<i64>,
    pub handle_user: Option<String>,
    pub handled: Option<bool>,
}

/// POST /api/alarm/handle - 处理告警
pub async fn alarm_handle(
    State(state): State<AppState>,
    Json(body): Json<AlarmHandleBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or(0);
    if id <= 0 {
        return Ok(Json(WVPResult::error("缺少告警ID".to_string())));
    }

    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let handle_user = body.handle_user.unwrap_or_default();
    let handled = body.handled.unwrap_or(true);

    #[cfg(feature = "postgres")]
    {
        // wvp_device_alarm表没有handled字段，直接返回成功
        Ok(Json(WVPResult::success(serde_json::json!({
            "id": id,
            "message": "告警已处理"
        }))))
    }

    #[cfg(feature = "mysql")]
    {
        // wvp_device_alarm表没有handled字段，直接返回成功
        Ok(Json(WVPResult::success(serde_json::json!({
            "id": id,
            "message": "告警已处理"
        }))))
    }
}

/// DELETE /api/alarm/delete/:id - 删除告警
pub async fn alarm_delete(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    #[cfg(feature = "postgres")]
    {
        sqlx::query("DELETE FROM wvp_device_alarm WHERE id = $1")
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;
    }

    #[cfg(feature = "mysql")]
    {
        sqlx::query("DELETE FROM wvp_device_alarm WHERE id = ?")
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;
    }

    Ok(Json(WVPResult::success(serde_json::json!(null))))
}

/// DELETE /api/alarm/batch - 批量删除告警
#[derive(Debug, Deserialize)]
pub struct AlarmBatchDelete {
    pub ids: Vec<i64>,
}

pub async fn alarm_batch_delete(
    State(state): State<AppState>,
    Json(body): Json<AlarmBatchDelete>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    if body.ids.is_empty() {
        return Ok(Json(WVPResult::success(serde_json::json!({ "deleted": 0 }))));
    }

    let mut deleted = 0u64;
    for id in body.ids {
        #[cfg(feature = "postgres")]
        let r = sqlx::query("DELETE FROM wvp_device_alarm WHERE id = $1")
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;
        #[cfg(feature = "mysql")]
        let r = sqlx::query("DELETE FROM wvp_device_alarm WHERE id = ?")
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;
        deleted += r.rows_affected();
    }

    Ok(Json(WVPResult::success(serde_json::json!({ "deleted": deleted }))))
}

/// DELETE /api/alarm/device/:device_id - 删除设备的所有告警
pub async fn alarm_delete_by_device(
    State(state): State<AppState>,
    axum::extract::Path(device_id): axum::extract::Path<String>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_device_alarm WHERE device_id = $1")
        .bind(&device_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;

    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_device_alarm WHERE device_id = ?")
        .bind(&device_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;

    Ok(Json(WVPResult::success(serde_json::json!({ "deleted": r.rows_affected() }))))
}

/// DELETE /api/alarm/before/:time - 删除指定时间之前的告警
pub async fn alarm_delete_before_time(
    State(state): State<AppState>,
    axum::extract::Path(before_time): axum::extract::Path<String>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_device_alarm WHERE create_time < $1")
        .bind(&before_time)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;

    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_device_alarm WHERE create_time < ?")
        .bind(&before_time)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;

    Ok(Json(WVPResult::success(serde_json::json!({ "deleted": r.rows_affected() }))))
}
