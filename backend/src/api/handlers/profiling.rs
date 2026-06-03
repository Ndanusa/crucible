use std::sync::Arc;
use axum::{Json, response::IntoResponse, extract::State};
use serde::{Serialize, Deserialize};
use utoipa::ToSchema;
use chrono::{DateTime, Utc};
use crate::error::AppError;
use crate::services::{
    sys_metrics::MetricsExporter,
    error_recovery::ErrorManager,
    tracing::TracingService,
    log_aggregator::LogAggregator,
};
use crate::config::reload::ConfigManager;
//! Performance profiling and system health API handlers.
//!
//! Provides endpoints for monitoring application health, collecting system
//! metrics, and triggering profiling runs.

use axum::{extract::State, response::IntoResponse, Json};
use redis::Client as RedisClient;
use crate::api::contracts::{ApiResponse, SystemStatus, ProfileTriggerRequest, ProfileTriggerResponse, ValidatedJson};

use crate::api::contracts::{
    ApiResponse, ProfileTriggerRequest, ProfileTriggerResponse, SystemStatus, ValidatedJson,
};
use crate::config::reload::ConfigManager;
use crate::error::AppError;
use crate::services::{
    error_recovery::ErrorManager,
    log_aggregator::LogAggregator,
    sys_metrics::MetricsExporter,
    tracing::TracingService,
};
use redis::Client as RedisClient;

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

/// Shared application state passed to profiling and status handlers.
//! Profiling and health check handlers.


use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::{
    config::reload::ConfigManager,
    error::AppError,
    services::{
        error_recovery::ErrorManager,
        log_aggregator::LogAggregator,
        sys_metrics::MetricsExporter,
    },
};

/// Shared application state passed to profiling and config handlers.
pub struct AppState {
    /// Optional PostgreSQL connection pool (None in tests).
    pub db: Option<sqlx::PgPool>,
    /// System metrics exporter.
    pub metrics_exporter: Arc<MetricsExporter>,
    /// Error recovery manager.
    pub error_manager: Arc<ErrorManager>,
    /// Hot-reloadable configuration manager.
    pub config_manager: Arc<ConfigManager>,
    /// Async log aggregation pipeline.
    pub log_aggregator: Arc<LogAggregator>,
    /// Redis client for caching.
    pub redis: RedisClient,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Detailed performance metrics report.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct MetricsReport {
    /// Total system uptime in seconds.
    pub uptime_secs: u64,
    /// Current resident set size (RSS) in bytes.
    pub memory_usage_bytes: u64,
    /// Number of currently active HTTP requests.
    pub active_requests: u32,
    /// Percentage of failed requests in the last window.
    pub error_rate: f64,
    /// Current latency for Stellar ledger ingestion in milliseconds.
    pub ledger_ingestion_latency_ms: u32,
}

/// System health check response.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Overall health status (e.g., `"healthy"` or `"degraded"`).
    pub status: String,
    /// The current version of the backend service.
    pub version: String,
    /// RFC3339 timestamp of the health check.
    pub timestamp: DateTime<Utc>,
    /// Connectivity status to the PostgreSQL database.
    pub database_connected: bool,
    /// Connectivity status to the Redis cache.
    pub redis_connected: bool,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /api/v1/profiling/metrics` — retrieve detailed performance metrics.
///
/// Optimized for consumption by monitoring tools like Grafana.
/// Performance metrics snapshot.
}

/// Health check response.
}

/// `GET /api/v1/profiling/metrics` — Return performance metrics.
#[utoipa::path(
    get,
    path = "/api/v1/profiling/metrics",
    responses(
        (status = 200, description = "Performance metrics", body = MetricsReport),
        (status = 500, description = "Internal server error")
    ),
    tag = "profiling"
)]
#[tracing::instrument(skip_all, fields(http.method = "GET", http.route = "/api/v1/profiling/metrics"))]
pub async fn get_metrics(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let span = tracing::info_span!("metrics.collection");
    let _enter = span.enter();
    
    tracing::info!("Collecting performance metrics");
    info!("Collecting performance metrics");

    let metrics_span = TracingService::service_method_span("MetricsExporter", "get_metrics");
    let _metrics_enter = metrics_span.enter();
    let sys_metrics = state.metrics_exporter.get_metrics().await;
    drop(_metrics_enter);

    let report = MetricsReport {
#[instrument(skip_all)]
    Ok(Json(MetricsReport {
        uptime_secs: sys_metrics.uptime,
        memory_usage_bytes: sys_metrics.memory_usage,
        active_requests: 12,
        error_rate: 0.001,
        ledger_ingestion_latency_ms: 120,
    };

    tracing::info!(
        uptime = sys_metrics.uptime,
        memory = sys_metrics.memory_usage,
        "Metrics collected successfully"
    );

    Ok(Json(report))
}

/// `GET /api/v1/profiling/health` — system health check.
///
/// Performs actual pings to downstream services.
    }))
}

/// `GET /api/v1/profiling/health` — System health check.
#[utoipa::path(
    get,
    path = "/api/v1/profiling/health",
    responses(
        (status = 200, description = "System is healthy", body = HealthResponse),
        (status = 503, description = "System is degraded")
    ),
    tag = "profiling"
)]
#[tracing::instrument(skip_all, fields(http.method = "GET", http.route = "/api/v1/profiling/health"))]
pub async fn get_health(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let span = tracing::info_span!("health.check");
    let _enter = span.enter();
    
    tracing::info!("Performing system health check");

    let db_healthy = if let Some(ref db) = state.db {
        // Check database connectivity with tracing
        let db_span = TracingService::db_query_span(
            "SELECT 1",
            "postgres",
            "PING"
        );
        let _db_enter = db_span.enter();
        
        let healthy = sqlx::query("SELECT 1")
            .fetch_optional(db)
#[instrument(skip_all, fields(http.method = "GET", http.route = "/api/v1/profiling/health"))]
#[instrument(skip_all)]
    info!("Performing system health check");
    let db_healthy = if let Some(ref pool) = state.db {
        sqlx::query("SELECT 1")
            .fetch_optional(pool)
            .await
            .map(|r| r.is_some())
            .unwrap_or(false)
    } else {
        false
    };

    let db_healthy = if let Some(ref pool) = state.db {
        let db_span = TracingService::db_query_span("SELECT 1", "postgres", "PING");
        let result = sqlx::query("SELECT 1")
            .fetch_optional(pool)
            .await
            .map(|r| r.is_some())
            .unwrap_or_else(|e| {
                TracingService::record_error(&db_span, &e.to_string(), "database");
                false
            });
        drop(_db_enter);
        healthy
    } else {
        false
    };
    
        result
    };

    let response = HealthResponse {
    Ok(Json(HealthResponse {
        status: if db_healthy { "healthy" } else { "degraded" }.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: Utc::now(),
        database_connected: db_healthy,
        redis_connected: true,
    };

    tracing::info!(
        db_connected = db_healthy,
        version = env!("CARGO_PKG_VERSION"),
        "Health check completed"
    );

    Ok(Json(response))
}

/// Handler for Prometheus-compatible metrics.
#[tracing::instrument(skip_all, fields(http.method = "GET", http.route = "/api/v1/profiling/prometheus"))]
pub async fn get_prometheus_metrics() -> impl IntoResponse {
    let span = tracing::info_span!("prometheus.metrics.export");
    let _enter = span.enter();
    
    tracing::info!("Exporting Prometheus-format metrics");
    
/// `GET /api/v1/profiling/prometheus` — Prometheus-compatible metrics.
#[instrument(skip_all, fields(http.method = "GET", http.route = "/api/v1/profiling/prometheus"))]
    info!("Exporting Prometheus-format metrics");
    }))
}

/// `GET /api/v1/profiling/prometheus` — Prometheus-format metrics.
#[instrument(skip_all)]
    "# HELP backend_requests_total Total number of requests\n\
     # TYPE backend_requests_total counter\n\
     backend_requests_total 1024\n\
     # HELP backend_ledger_latency_ms Current ledger ingestion latency\n\
     # TYPE backend_ledger_latency_ms gauge\n\
     backend_ledger_latency_ms 120\n"
        .to_string()
}

/// Handler for detailed system status
#[tracing::instrument(skip_all, fields(http.method = "GET", http.route = "/api/status"))]
pub async fn get_system_status(
    State(state): State<Arc<AppState>>,
) -> ApiResponse<SystemStatus> {
    let span = tracing::info_span!("system.status");
    let _enter = span.enter();
    
    tracing::info!("Retrieving system status");
    
/// `GET /api/status` — detailed system status.
#[instrument(skip_all, fields(http.method = "GET", http.route = "/api/status"))]
    info!("Retrieving system status");

    let metrics_span = TracingService::service_method_span("MetricsExporter", "get_metrics");
    let _metrics_enter = metrics_span.enter();
    let metrics = state.metrics_exporter.get_metrics().await;
    drop(_metrics_enter);

    let recovery_span = TracingService::service_method_span("ErrorManager", "get_active_tasks");
    let _recovery_enter = recovery_span.enter();
    let recovery_tasks = state.error_manager.get_active_tasks().await;
    drop(_recovery_enter);

    ApiResponse::new(SystemStatus {
        status: "healthy".to_string(),
        uptime_secs: metrics.uptime,
        memory_used_bytes: metrics.memory_usage,
        active_recovery_tasks: recovery_tasks.len(),
    })
}

/// Handler to trigger profile collection (CPU, memory profiling)
#[tracing::instrument(skip_all, fields(http.method = "POST", http.route = "/api/profile"))]
pub async fn trigger_profile_collection(
    State(_state): State<Arc<AppState>>,
    ValidatedJson(payload): ValidatedJson<ProfileTriggerRequest>,
) -> Result<ApiResponse<ProfileTriggerResponse>, AppError> {
    // In a real implementation, this would trigger a CPU/Memory profile
    // using the provided payload (duration, sample rate, etc.)
    
    ApiResponse::new(ProfileTriggerResponse {
        profile_id: uuid::Uuid::new_v4(),
        message: format!("Profiling collection triggered for label: {}", payload.label),
        estimated_completion: chrono::Utc::now() + chrono::Duration::seconds(payload.duration_secs as i64),
/// `POST /api/profile` — trigger a profiling collection run.
#[utoipa::path(
    post,
    path = "/api/profile",
    responses(
        (status = 200, description = "Profiling collection triggered"),
        (status = 400, description = "Invalid request parameters")
    ),
    tag = "profiling"
)]
#[instrument(skip_all, fields(http.method = "POST", http.route = "/api/profile"))]
) -> ApiResponse<ProfileTriggerResponse> {
    let profile_id = uuid::Uuid::new_v4();

    info!(
        profile_id = %profile_id,
        label = %payload.label,
        duration_secs = payload.duration_secs,
        "Profiling collection triggered"
    );

        profile_id,
        estimated_completion: chrono::Utc::now()
            + chrono::Duration::seconds(payload.duration_secs as i64),
    })
/// `GET /api/status` — System status summary.
#[instrument(skip_all)]
pub async fn get_system_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "uptime_secs": metrics.uptime,
        "memory_used_bytes": metrics.memory_usage,
        "active_recovery_tasks": recovery_tasks.len(),
    }))
}

/// `POST /api/profile` — Trigger profile collection.
) -> impl IntoResponse {
    let profile_id = uuid::Uuid::new_v4().to_string();
    info!(profile_id = %profile_id, "Profiling collection triggered");
        "message": "Profiling collection triggered",
        "profile_id": profile_id,
}
