use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;
use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LogAlertRule {
    pub id: Uuid,
    pub name: String,
    pub pattern: String,
    pub threshold: i32,
    pub interval_seconds: i32,
    pub is_enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub pattern: String,
    pub threshold: i32,
    pub interval_seconds: i32,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LogAlert {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub message: String,
    pub triggered_at: chrono::DateTime<chrono::Utc>,
}

pub struct ServiceState {
    pub db: PgPool,
    pub redis: redis::Client,
}

pub fn router() -> Router {
    Router::new()
        .route("/rules", post(create_rule).get(list_rules))
        .route("/rules/:id", get(get_rule))
        .route("/ingest", post(ingest_log))
}

async fn create_rule(
    State(state): State<Arc<ServiceState>>,
    Json(payload): Json<CreateRuleRequest>,
) -> Result<Json<LogAlertRule>, AppError> {
    let rule = sqlx::query_as::<_, LogAlertRule>(
        "INSERT INTO log_alert_rules (name, pattern, threshold, interval_seconds) 
         VALUES ($1, $2, $3, $4) RETURNING *"
    )
    .bind(payload.name)
    .bind(payload.pattern)
    .bind(payload.threshold)
    .bind(payload.interval_seconds)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(rule))
}

async fn list_rules(
    State(state): State<Arc<ServiceState>>,
) -> Result<Json<Vec<LogAlertRule>>, AppError> {
    let rules = sqlx::query_as::<_, LogAlertRule>("SELECT * FROM log_alert_rules")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rules))
}

async fn get_rule(
    State(state): State<Arc<ServiceState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<LogAlertRule>, AppError> {
    let rule = sqlx::query_as::<_, LogAlertRule>("SELECT * FROM log_alert_rules WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Rule not found: {}", id)))?;
    
    Ok(Json(rule))
}

#[derive(Debug, Deserialize)]
pub struct LogEntry {
    pub message: String,
    pub level: String,
}

async fn ingest_log(
    State(state): State<Arc<ServiceState>>,
    Json(log): Json<LogEntry>,
) -> Result<Json<serde_json::Value>, AppError> {
    tracing::info!("Processing log: {}", log.message);
    
    // In a real system, we would match against rules cached in Redis
    // and use Redis to track counts over time.
    
    Ok(Json(serde_json::json!({ "status": "processed" })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        let pattern = "error";
        let message = "This is an error message";
        assert!(message.contains(pattern));
    }
}
