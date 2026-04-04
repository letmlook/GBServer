use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::error::AppError;
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
                    alarm_time, alarm_description, handled, handle_time, handle_user 
             FROM wvp_alarm 
             WHERE ($1::text IS NULL OR device_id = $1)
               AND ($2::text IS NULL OR channel_id = $2)
               AND ($3::text IS NULL OR alarm_type = $3)
               AND ($4::text IS NULL OR alarm_method = $4)
               AND ($5::boolean IS NULL OR handled = $5)
             ORDER BY alarm_time DESC 
             LIMIT $6 OFFSET $7",
        )
        .bind(q.device_id)
        .bind(q.channel_id)
        .bind(q.alarm_type)
        .bind(q.alarm_method)
        .bind(q.handled)
        .bind(count as i64)
        .bind(offset as i64)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("数据库查询失败: {}", e)))?
        .into_iter()
        .map(|r| {
            let id: i64 = r.get("id");
            let device_id: Option<String> = r.get("device_id");
            let channel_id: Option<String> = r.get("channel_id");
            let alarm_priority: Option<i32> = r.get("alarm_priority");
            let alarm_method: Option<String> = r.get("alarm_method");
            let alarm_type: Option<String> = r.get("alarm_type");
            let alarm_time: Option<String> = r.get("alarm_time");
            let alarm_description: Option<String> = r.get("alarm_description");
            let handled: Option<bool> = r.get("handled");
            let handle_time: Option<String> = r.get("handle_time");
            let handle_user: Option<String> = r.get("handle_user");
            serde_json::json!({
                "id": id,
                "deviceId": device_id,
                "channelId": channel_id,
                "alarmPriority": alarm_priority,
                "alarmMethod": alarm_method,
                "alarmType": alarm_type,
                "alarmTime": alarm_time,
                "alarmDescription": alarm_description,
                "handled": handled,
                "handleTime": handle_time,
                "handleUser": handle_user,
            })
        })
        .collect();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM wvp_alarm 
             WHERE ($1::text IS NULL OR device_id = $1)
               AND ($2::text IS NULL OR channel_id = $2)
               AND ($3::text IS NULL OR alarm_type = $3)
               AND ($4::text IS NULL OR alarm_method = $4)
               AND ($5::boolean IS NULL OR handled = $5)",
        )
        .bind(q.device_id)
        .bind(q.channel_id)
        .bind(q.alarm_type)
        .bind(q.alarm_method)
        .bind(q.handled)
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
                    alarm_time, alarm_description, handled, handle_time, handle_user 
             FROM wvp_alarm 
             WHERE (? IS NULL OR device_id = ?)
               AND (? IS NULL OR channel_id = ?)
               AND (? IS NULL OR alarm_type = ?)
               AND (? IS NULL OR alarm_method = ?)
               AND (? IS NULL OR handled = ?)
             ORDER BY alarm_time DESC 
             LIMIT ? OFFSET ?",
        )
        .bind(&q.device_id).bind(&q.device_id)
        .bind(&q.channel_id).bind(&q.channel_id)
        .bind(&q.alarm_type).bind(&q.alarm_type)
        .bind(&q.alarm_method).bind(&q.alarm_method)
        .bind(q.handled)
        .bind(count as i64)
        .bind(offset as i64)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("数据库查询失败: {}", e)))?
        .into_iter()
        .map(|r| {
            let id: i64 = r.get("id");
            let device_id: Option<String> = r.get("device_id");
            let channel_id: Option<String> = r.get("channel_id");
            let alarm_priority: Option<i32> = r.get("alarm_priority");
            let alarm_method: Option<String> = r.get("alarm_method");
            let alarm_type: Option<String> = r.get("alarm_type");
            let alarm_time: Option<String> = r.get("alarm_time");
            let alarm_description: Option<String> = r.get("alarm_description");
            let handled: Option<bool> = r.get("handled");
            let handle_time: Option<String> = r.get("handle_time");
            let handle_user: Option<String> = r.get("handle_user");
            serde_json::json!({
                "id": id,
                "deviceId": device_id,
                "channelId": channel_id,
                "alarmPriority": alarm_priority,
                "alarmMethod": alarm_method,
                "alarmType": alarm_type,
                "alarmTime": alarm_time,
                "alarmDescription": alarm_description,
                "handled": handled,
                "handleTime": handle_time,
                "handleUser": handle_user,
            })
        })
        .collect();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM wvp_alarm 
             WHERE (? IS NULL OR device_id = ?)
               AND (? IS NULL OR channel_id = ?)
               AND (? IS NULL OR alarm_type = ?)
               AND (? IS NULL OR alarm_method = ?)
               AND (? IS NULL OR handled = ?)",
        )
        .bind(&q.device_id).bind(&q.device_id)
        .bind(&q.channel_id).bind(&q.channel_id)
        .bind(&q.alarm_type).bind(&q.alarm_type)
        .bind(&q.alarm_method).bind(&q.alarm_method)
        .bind(q.handled)
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
            "SELECT * FROM wvp_alarm WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("数据库查询失败: {}", e)))?;

        match row {
            Some(r) => {
                let id: i64 = r.get("id");
                let device_id: Option<String> = r.get("device_id");
                let channel_id: Option<String> = r.get("channel_id");
                let alarm_priority: Option<i32> = r.get("alarm_priority");
                let alarm_method: Option<String> = r.get("alarm_method");
                let alarm_type: Option<String> = r.get("alarm_type");
                let alarm_time: Option<String> = r.get("alarm_time");
                let alarm_description: Option<String> = r.get("alarm_description");
                let handled: Option<bool> = r.get("handled");
                let handle_time: Option<String> = r.get("handle_time");
                let handle_user: Option<String> = r.get("handle_user");
                Ok(Json(WVPResult::success(serde_json::json!({
                    "id": id,
                    "deviceId": device_id,
                    "channelId": channel_id,
                    "alarmPriority": alarm_priority,
                    "alarmMethod": alarm_method,
                    "alarmType": alarm_type,
                    "alarmTime": alarm_time,
                    "alarmDescription": alarm_description,
                    "handled": handled,
                    "handleTime": handle_time,
                    "handleUser": handle_user,
                }))))
            }
            None => Ok(Json(WVPResult::error("告警不存在".to_string()))),
        }
    }

    #[cfg(feature = "mysql")]
    {
        let row = sqlx::query("SELECT * FROM wvp_alarm WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("数据库查询失败: {}", e)))?;

        match row {
            Some(r) => {
                let id: i64 = r.get("id");
                let device_id: Option<String> = r.get("device_id");
                let channel_id: Option<String> = r.get("channel_id");
                let alarm_priority: Option<i32> = r.get("alarm_priority");
                let alarm_method: Option<String> = r.get("alarm_method");
                let alarm_type: Option<String> = r.get("alarm_type");
                let alarm_time: Option<String> = r.get("alarm_time");
                let alarm_description: Option<String> = r.get("alarm_description");
                let handled: Option<bool> = r.get("handled");
                let handle_time: Option<String> = r.get("handle_time");
                let handle_user: Option<String> = r.get("handle_user");
                Ok(Json(WVPResult::success(serde_json::json!({
                    "id": id,
                    "deviceId": device_id,
                    "channelId": channel_id,
                    "alarmPriority": alarm_priority,
                    "alarmMethod": alarm_method,
                    "alarmType": alarm_type,
                    "alarmTime": alarm_time,
                    "alarmDescription": alarm_description,
                    "handled": handled,
                    "handleTime": handle_time,
                    "handleUser": handle_user,
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
        sqlx::query(
            "UPDATE wvp_alarm SET handled = $1, handle_time = $2, handle_user = $3 WHERE id = $4",
        )
        .bind(handled)
        .bind(&now)
        .bind(&handle_user)
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("数据库更新失败: {}", e)))?;
    }

    #[cfg(feature = "mysql")]
    {
        sqlx::query(
            "UPDATE wvp_alarm SET handled = ?, handle_time = ?, handle_user = ? WHERE id = ?",
        )
        .bind(handled)
        .bind(&now)
        .bind(&handle_user)
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("数据库更新失败: {}", e)))?;
    }

    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "message": "告警已处理"
    }))))
}

/// DELETE /api/alarm/delete/:id - 删除告警
pub async fn alarm_delete(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    #[cfg(feature = "postgres")]
    {
        sqlx::query("DELETE FROM wvp_alarm WHERE id = $1")
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("数据库删除失败: {}", e)))?;
    }

    #[cfg(feature = "mysql")]
    {
        sqlx::query("DELETE FROM wvp_alarm WHERE id = ?")
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("数据库删除失败: {}", e)))?;
    }

    Ok(Json(WVPResult::success_empty()))
}
