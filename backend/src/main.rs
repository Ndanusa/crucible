//! # Crucible Backend
//!
//! Production-ready HTTP API server for the Crucible smart contract testing
//! platform. Built with [Axum](https://docs.rs/axum), [SQLx](https://docs.rs/sqlx)
//! (PostgreSQL), and [Redis](https://docs.rs/redis) for caching and job queues.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────┐     ┌──────────────┐     ┌────────────┐
//! │  Client   │────▶│  Axum Router  │────▶│ PostgreSQL │
//! └──────────┘     │  (port 8080)  │     └────────────┘
//!                  │               │     ┌────────────┐
//!                  │  Middleware:   │────▶│   Redis    │
//!                  │  - CORS       │     └────────────┘
//!                  │  - Tracing    │
//!                  │  - Compression│
//!                  └──────────────┘
//! ```

use std::net::SocketAddr;
use std::time::Duration;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use redis::aio::ConnectionManager;
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

pub mod error;

/// Shared application state passed to all handlers via Axum's state extraction.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool managed by SQLx.
    pub db: PgPool,
    /// Redis connection manager for caching and job queues.
    pub redis: ConnectionManager,
}

/// Response returned by the `/health` endpoint.
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    database: String,
    redis: String,
}

#[tokio::main]
async fn main() {
    // Load .env file if present (development convenience)
    dotenvy::dotenv().ok();

    // Initialize structured logging with tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "crucible_backend=debug,tower_http=debug".into()),
        )
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting Crucible Backend");

    // ----- Database connection -----
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let max_connections: u32 = std::env::var("DATABASE_MAX_CONNECTIONS")
        .unwrap_or_else(|_| "10".into())
        .parse()
        .expect("DATABASE_MAX_CONNECTIONS must be a valid u32");

    let min_connections: u32 = std::env::var("DATABASE_MIN_CONNECTIONS")
        .unwrap_or_else(|_| "2".into())
        .parse()
        .expect("DATABASE_MIN_CONNECTIONS must be a valid u32");

    let db = PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(min_connections)
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(600))
        .test_before_acquire(true)
        .connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL");

    info!("Connected to PostgreSQL (pool: {min_connections}..{max_connections})");

    // Run pending migrations
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("Failed to run database migrations");

    info!("Database migrations applied");

    // ----- Redis connection -----
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".into());

    let redis_client = redis::Client::open(redis_url.as_str())
        .expect("Invalid REDIS_URL");

    let redis = ConnectionManager::new(redis_client)
        .await
        .expect("Failed to connect to Redis");

    info!("Connected to Redis");

    // ----- Application state -----
    let state = AppState { db, redis };

    // ----- Router -----
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/status", get(api_status))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    // ----- Server -----
    let host = std::env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("APP_PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()
        .expect("APP_PORT must be a valid u16");

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .expect("Invalid APP_HOST:APP_PORT combination");

    info!("Listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind TCP listener");

    axum::serve(listener, app.into_make_service())
        .await
        .expect("Server error");
}

/// `GET /health` — Comprehensive health check for load balancers and Docker.
///
/// Verifies connectivity to both PostgreSQL and Redis, returning a JSON
/// response with individual service statuses.
async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let db_status = match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        Ok(_) => "healthy".to_string(),
        Err(e) => {
            warn!("Database health check failed: {e}");
            format!("unhealthy: {e}")
        }
    };

    let redis_status = {
        let mut conn = state.redis.clone();
        match redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
        {
            Ok(_) => "healthy".to_string(),
            Err(e) => {
                warn!("Redis health check failed: {e}");
                format!("unhealthy: {e}")
            }
        }
    };

    let all_healthy = db_status == "healthy" && redis_status == "healthy";
    let status_code = if all_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(HealthResponse {
            status: if all_healthy {
                "ok".into()
            } else {
                "degraded".into()
            },
            version: env!("CARGO_PKG_VERSION").into(),
            database: db_status,
            redis: redis_status,
        }),
    )
}

/// `GET /api/v1/status` — Simple API status endpoint.
async fn api_status() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "crucible-backend",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "running"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "ok".into(),
            version: "0.1.0".into(),
            database: "healthy".into(),
            redis: "healthy".into(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"status\":\"ok\""));
        assert!(json.contains("\"database\":\"healthy\""));
        assert!(json.contains("\"redis\":\"healthy\""));
    }

    #[test]
    fn test_health_response_fields() {
        let response = HealthResponse {
            status: "degraded".into(),
            version: "0.1.0".into(),
            database: "healthy".into(),
            redis: "unhealthy: connection refused".into(),
        };
        assert_eq!(response.status, "degraded");
        assert_eq!(response.database, "healthy");
        assert!(response.redis.starts_with("unhealthy"));
    }
}
