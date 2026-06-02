use axum::{Json, response::IntoResponse, extract::{State, Path}};
use serde::{Serialize, Deserialize};
use tracing::{info, instrument, error, debug, warn};
use chrono::{DateTime, Utc};
use crate::error::AppError;
use utoipa::ToSchema;
use std::sync::Arc;
use sqlx::PgPool;
use redis::{AsyncCommands, Client as RedisClient};
use thiserror::Error;

use crate::services::{
    error_recovery::{ErrorManager, RecoveryTask},
    log_alerts::{Alert, AlertManager},
    sys_metrics::{MetricsExporter, SystemMetrics},
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const CACHE_KEY: &str = "dashboard:summary";
const CACHE_TTL_SECS: u64 = 30;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------
pub struct DashboardState {
    pub db: PgPool,
    pub redis_conn: redis::aio::ConnectionManager,
    pub metrics_exporter: Arc<MetricsExporter>,
    pub error_manager: Arc<ErrorManager>,
    pub alert_manager: Arc<AlertManager>,
    pub redis_client: RedisClient,
}

// ---------------------------------------------------------------------------
// Error type for get_dashboard
// ---------------------------------------------------------------------------
#[derive(Debug, Error)]
pub enum DashboardError {
    #[error("Cache error: {0}")]
    Cache(#[from] redis::RedisError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl IntoResponse for DashboardError {
    fn into_response(self) -> axum::response::Response {
        error!(error = %self, "Dashboard handler error");
        let body = serde_json::json!({ "error": self.to_string() });
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
    }
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct DashboardMetrics {
    /// Total number of active contracts
    pub total_contracts: i64,
    /// Total number of transactions processed
    pub total_transactions: i64,
    /// Average transaction processing time in milliseconds
    pub avg_processing_time_ms: f64,
    /// Number of failed transactions in the last 24 hours
    pub failed_transactions_24h: i64,
    /// Timestamp of the metrics snapshot
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ContractStats {
    /// Contract identifier
    pub contract_id: String,
    /// Number of invocations
    pub invocation_count: i64,
    /// Last invocation timestamp
    pub last_invoked: Option<DateTime<Utc>>,
    /// Average gas cost
    pub avg_gas_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    /// Current system metrics snapshot.
    pub metrics: SystemMetrics,
    /// Recovery tasks that are currently active.
    pub active_recovery_tasks: Vec<RecoveryTask>,
    /// Alerts that have fired and not yet been resolved.
    pub active_alerts: Vec<Alert>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Retrieves aggregated dashboard metrics with Redis caching
#[utoipa::path(
    get,
    path = "/api/v1/dashboard/metrics",
    responses(
        (status = 200, description = "Dashboard metrics retrieved successfully", body = DashboardMetrics),
        (status = 500, description = "Internal server error")
    ),
    tag = "dashboard"
)]
#[instrument(skip(state))]
pub async fn get_dashboard_metrics(
    State(state): State<Arc<DashboardState>>,
) -> Result<impl IntoResponse, AppError> {
    info!("Fetching dashboard metrics");

    // Try cache first
    let cache_key = "dashboard:metrics";
    let mut redis_conn = state.redis_conn.clone();
    
    if let Ok(cached) = redis_conn.get::<_, String>(cache_key).await {
        if let Ok(metrics) = serde_json::from_str::<DashboardMetrics>(&cached) {
            info!("Returning cached dashboard metrics");
            return Ok(Json(metrics));
        }
    }

    // Fetch from database
    let total_contracts = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM contracts"
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(0);

    let total_transactions = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM transactions"
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(0);

    let avg_processing_time = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT AVG(processing_time_ms) FROM transactions WHERE processing_time_ms IS NOT NULL"
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(0.0);

    let failed_24h = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM transactions 
         WHERE status = 'failed' AND created_at > NOW() - INTERVAL '24 hours'"
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(0);

    let metrics = DashboardMetrics {
        total_contracts,
        total_transactions,
        avg_processing_time_ms: avg_processing_time,
        failed_transactions_24h: failed_24h,
        timestamp: Utc::now(),
    };

    // Cache for 60 seconds
    if let Ok(json) = serde_json::to_string(&metrics) {
        let _: Result<(), _> = redis_conn.set_ex(cache_key, json, 60).await;
    }

    info!(
        contracts = metrics.total_contracts,
        transactions = metrics.total_transactions,
        "Dashboard metrics retrieved"
    );

    Ok(Json(metrics))
}

/// Retrieves statistics for a specific contract
#[utoipa::path(
    get,
    path = "/api/v1/dashboard/contracts/{contract_id}/stats",
    params(
        ("contract_id" = String, Path, description = "Contract identifier")
    ),
    responses(
        (status = 200, description = "Contract statistics retrieved", body = ContractStats),
        (status = 404, description = "Contract not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "dashboard"
)]
#[instrument(skip(state))]
pub async fn get_contract_stats(
    State(state): State<Arc<DashboardState>>,
    Path(contract_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    info!(contract_id = %contract_id, "Fetching contract statistics");

    let cache_key = format!("dashboard:contract:{}:stats", contract_id);
    let mut redis_conn = state.redis_conn.clone();

    // Check cache
    if let Ok(cached) = redis_conn.get::<_, String>(&cache_key).await {
        if let Ok(stats) = serde_json::from_str::<ContractStats>(&cached) {
            return Ok(Json(stats));
        }
    }

    // Query database — plain query() avoids compile-time DB verification
    let row: Option<(i64, Option<DateTime<Utc>>, Option<f64>)> = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as invocation_count,
            MAX(created_at) as last_invoked,
            AVG(gas_cost) as avg_gas_cost
        FROM transactions
        WHERE contract_id = $1
        "#,
    )
    .bind(&contract_id)
    .fetch_optional(&state.db)
    .await?;

    let stats = match row {
        Some((invocation_count, last_invoked, avg_gas_cost)) if invocation_count > 0 => ContractStats {
            contract_id: contract_id.clone(),
            invocation_count,
            last_invoked,
            avg_gas_cost: avg_gas_cost.unwrap_or(0.0),
        },
        _ => {
            error!(contract_id = %contract_id, "Contract not found");
            return Err(AppError::NotFound(format!("Contract {} not found", contract_id)));
        }
    };

    // Cache for 30 seconds
    if let Ok(json) = serde_json::to_string(&stats) {
        let _: Result<(), _> = redis_conn.set_ex(&cache_key, json, 30).await;
    }

    Ok(Json(stats))
}

/// `GET /api/dashboard` — return aggregated dashboard data.
#[tracing::instrument(skip(state))]
pub async fn get_dashboard(
    State(state): State<Arc<DashboardState>>,
) -> Result<impl IntoResponse, DashboardError> {
    // --- try cache ---
    match try_cache_get(&state.redis_client).await {
        Ok(Some(cached)) => {
            debug!("Dashboard cache hit");
            return Ok(Json(cached));
        }
        Ok(None) => debug!("Dashboard cache miss"),
        Err(e) => warn!(error = %e, "Dashboard cache read failed; falling back to live data"),
    }

    // --- assemble live data ---
    let (metrics, active_recovery_tasks, active_alerts) = tokio::join!(
        state.metrics_exporter.get_metrics(),
        state.error_manager.get_active_tasks(),
        state.alert_manager.get_active_alerts(),
    );

    let data = DashboardData {
        metrics,
        active_recovery_tasks,
        active_alerts,
    };

    // --- populate cache (best-effort) ---
    if let Err(e) = try_cache_set(&state.redis_client, &data).await {
        warn!(error = %e, "Failed to populate dashboard cache");
    }

    Ok(Json(data))
}

/// `GET /api/v1/dashboard/metrics` — return aggregate contract and pipeline metrics.
#[tracing::instrument(skip(state))]
pub async fn get_dashboard_metrics(
    State(state): State<Arc<DashboardState>>,
) -> Result<impl IntoResponse, DashboardError> {
    match try_cache_get(&state.redis, "dashboard:metrics").await {
        Ok(Some(cached)) => return Ok(Json(cached)),
        Ok(None) => debug!("Dashboard metrics cache miss"),
        Err(e) => {
            warn!(error = %e, "Dashboard metrics cache read failed; falling back to live data")
        }
    }

    let total_contracts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM contracts")
        .fetch_one(&state.db)
        .await?;
    let total_transactions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
        .fetch_one(&state.db)
        .await?;
    let avg_processing_time_ms: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(processing_time_ms) FROM transactions WHERE processing_time_ms IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await?;
    let failed_transactions_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM transactions WHERE status = 'failed' AND created_at >= NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(&state.db)
    .await?;

    let metrics = DashboardMetrics {
        total_contracts: total_contracts as u64,
        total_transactions: total_transactions as u64,
        avg_processing_time_ms: avg_processing_time_ms.unwrap_or(0.0),
        failed_transactions_24h: failed_transactions_24h as u64,
        timestamp: Utc::now(),
    };

    if let Err(e) = try_cache_set(&state.redis, "dashboard:metrics", &metrics).await {
        warn!(error = %e, "Failed to populate dashboard metrics cache");
    }

    Ok(Json(metrics))
}

/// `GET /api/v1/dashboard/contracts/:contract_id/stats` — return contract usage statistics.
#[tracing::instrument(skip(state))]
pub async fn get_contract_stats(
    Path(contract_id): Path<String>,
    State(state): State<Arc<DashboardState>>,
) -> Result<impl IntoResponse, DashboardError> {
    let cache_key = format!("dashboard:contract_stats:{contract_id}");

    match try_cache_get(&state.redis, &cache_key).await {
        Ok(Some(cached)) => return Ok(Json(cached)),
        Ok(None) => debug!(contract_id = %contract_id, "Contract stats cache miss"),
        Err(e) => {
            warn!(error = %e, contract_id = %contract_id, "Contract stats cache read failed; falling back to live data")
        }
    }

    let exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM contracts WHERE contract_id = $1")
        .bind(&contract_id)
        .fetch_optional(&state.db)
        .await?;

    if exists.is_none() {
        return Err(DashboardError::NotFound(format!(
            "Contract {contract_id} not found"
        )));
    }

    let (invocation_count, last_invoked, avg_gas_cost): (i64, Option<DateTime<Utc>>, Option<f64>) =
        sqlx::query_as(
            "SELECT COUNT(*), MAX(created_at), AVG(gas_cost) FROM transactions WHERE contract_id = $1",
        )
        .bind(&contract_id)
        .fetch_one(&state.db)
        .await?;

    let stats = ContractStats {
        contract_id,
        invocation_count: invocation_count as u64,
        last_invoked,
        avg_gas_cost: avg_gas_cost.unwrap_or(0.0),
    };

    if let Err(e) = try_cache_set(&state.redis, &cache_key, &stats).await {
        warn!(error = %e, contract_id = %stats.contract_id, "Failed to populate contract stats cache");
    }

    Ok(Json(stats))
}

// ---------------------------------------------------------------------------
// Cache helpers
// ---------------------------------------------------------------------------
async fn try_cache_get(redis: &RedisClient) -> Result<Option<DashboardData>, DashboardError> {
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let raw: Option<String> = conn.get(key).await?;
    match raw {
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
        None => Ok(None),
    }
}

async fn try_cache_set<T>(redis: &RedisClient, key: &str, data: &T) -> Result<(), DashboardError>
where
    T: Serialize,
{
    let serialized = serde_json::to_string(data)?;
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let _: () = conn.set_ex(key, serialized, CACHE_TTL_SECS).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    fn make_state(db: PgPool, redis_conn: redis::aio::ConnectionManager) -> Arc<DashboardState> {
        Arc::new(DashboardState {
            db,
            redis_conn,
            metrics_exporter: Arc::new(MetricsExporter::new()),
            error_manager: Arc::new(ErrorManager::new()),
            alert_manager: Arc::new(AlertManager::new()),
            redis_client: RedisClient::open("redis://127.0.0.1:1/").unwrap(),
        })
    }

    #[test]
    fn test_dashboard_metrics_serialization() {
        let metrics = DashboardMetrics {
            total_contracts: 100,
            total_transactions: 5000,
            avg_processing_time_ms: 125.5,
            failed_transactions_24h: 3,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: DashboardMetrics = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.total_contracts, 100);
        assert_eq!(deserialized.total_transactions, 5000);
    }

    #[test]
    fn test_contract_stats_serialization() {
        let stats = ContractStats {
            contract_id: "test_contract_123".to_string(),
            invocation_count: 42,
            last_invoked: Some(Utc::now()),
            avg_gas_cost: 1500.75,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: ContractStats = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.contract_id, "test_contract_123");
        assert_eq!(deserialized.invocation_count, 42);
    }

    #[test]
    fn test_dashboard_error_display() {
        let err = DashboardError::Serialization(
            serde_json::from_str::<serde_json::Value>("bad json").unwrap_err(),
        );
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn test_dashboard_data_serialization_roundtrip() {
        let data = DashboardData {
            metrics: SystemMetrics::default(),
            active_recovery_tasks: vec![],
            active_alerts: vec![],
        };
        let json = serde_json::to_string(&data).unwrap();
        let back: DashboardData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.active_recovery_tasks.len(), 0);
        assert_eq!(back.active_alerts.len(), 0);
    }
}
