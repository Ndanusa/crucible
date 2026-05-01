mod api;
mod services;
mod config;

use std::sync::Arc;
use apalis::prelude::*;
use apalis_redis::RedisStorage;
use axum::{
    routing::{get, post},
    Router,
};
use backend::api::handlers::dashboard::{get_dashboard, DashboardState};
use backend::{
    api::handlers::{profiling, stellar},
    config::Config,
    jobs::{monitor_transaction, TransactionMonitorJob},
    config::Config,
    jobs::{monitor_transaction, TransactionMonitorJob},
    api::handlers::{profiling, stellar, dashboard},
    api::handlers::{profiling, stellar},
    api::middleware::logging::logging_middleware,
    services::{
        error_recovery::ErrorManager, log_aggregator::LogAggregator, log_alerts::AlertManager,
        sys_metrics::MetricsExporter,
        error_recovery::ErrorManager,
        log_aggregator::LogAggregator,
        tracing::{TracingService, TracingConfig},
    },
    telemetry::init_telemetry,
};
use profiling::AppState;
use redis::aio::ConnectionManager;
use sqlx::postgres::PgPoolOptions;
use axum::{routing::{get, post}, Router, middleware};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use crate::config::{AppConfig, reload::{ConfigManager, handle_reload, handle_get_config}};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use profiling::AppState;
use apalis::prelude::*;
use apalis_redis::RedisStorage;
use sqlx::postgres::PgPoolOptions;
use redis::aio::ConnectionManager;
use redis::Client as RedisClient;
use std::sync::Arc;
use tracing::info_span;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Load configuration
    let config = Config::from_env()?;

    // Initialize observability (fmt tracing subscriber)
    init_telemetry();

    // Database setup
    // Initialize OpenTelemetry tracing FIRST - before any other services
    let tracing_config = TracingConfig::new(
        "crucible-backend".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    )
    .with_environment(std::env::var("ENV").unwrap_or("dev".to_string()))
    .with_otlp_endpoint(
        std::env::var("OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4317".to_string())
    );

    TracingService::init(tracing_config)?;

    let span = info_span!("app.startup");
    let _enter = span.enter();

    // Database setup & migrations
    let db_span = TracingService::db_query_span(
        "CONNECT postgresql",
        "postgres",
        "CONNECT",
    );
    let _db_enter = db_span.enter();

    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;

    tracing::info!("Database connection established");
    
    tracing::info!("Database pool initialized");
    drop(_db_enter);

    let redis_client = RedisClient::open(config.redis_url.clone())?;

    // Initialize services
    let metrics_exporter = Arc::new(MetricsExporter::new());
    let error_manager = Arc::new(ErrorManager::new());
    let alert_manager = Arc::new(AlertManager::new());
    let (_log_aggregator, log_receiver) = LogAggregator::new();
    let (log_aggregator, log_receiver) = LogAggregator::new();
    let log_aggregator = Arc::new(log_aggregator);

    tokio::spawn(MetricsExporter::run_collector(metrics_exporter.clone()));
    tokio::spawn(LogAggregator::run_worker(log_receiver));
    
    // Initialize config manager
    let config = AppConfig::default();
    let config_manager = Arc::new(ConfigManager::new(config));

    // Redis + job queue setup
    // Redis Job Queue setup
    let conn = ConnectionManager::new(redis_client.clone()).await?;
    let redis_span = TracingService::redis_command_span("CONNECT", None);
    let _redis_enter = redis_span.enter();

    let redis_client = redis::Client::open(config.redis_url.clone())?;
    let conn = ConnectionManager::new(redis_client.clone()).await?;
    let redis_conn_dashboard = ConnectionManager::new(redis_client).await?;
    let storage: RedisStorage<TransactionMonitorJob> = RedisStorage::new(conn);

    
    tracing::info!("Redis connection established");
    drop(_redis_enter);
    
    let worker = WorkerBuilder::new("monitor-worker")
        .backend(storage)
        .build_fn(monitor_transaction);

    // Create shared state
    let state = Arc::new(AppState {
        db: db_pool.clone(),
    // Shared state for profiling/status routes
    let profiling_state = Arc::new(AppState {
        db: Some(db_pool),
        metrics_exporter: metrics_exporter.clone(),
        error_manager: error_manager.clone(),
    });

    // Shared state for dashboard route
    let dashboard_state = Arc::new(DashboardState {
        metrics_exporter,
        error_manager,
        config_manager: config_manager.clone(),
        alert_manager,
        log_aggregator,
        redis: redis_client,
    });

    // Create dashboard state
    let dashboard_state = Arc::new(dashboard::DashboardState {
        db: db_pool,
        redis: redis_conn_dashboard,
    });

    // Define OpenAPI documentation
    // OpenAPI docs
    #[derive(OpenApi)]
    #[openapi(
        paths(
            profiling::get_metrics,
            profiling::get_health,
            dashboard::get_dashboard_metrics,
            dashboard::get_contract_stats,
        ),
        components(
            schemas(
                profiling::MetricsReport, 
                profiling::HealthResponse,
                dashboard::DashboardMetrics,
                dashboard::ContractStats
            )
        ),
        tags(
            (name = "profiling", description = "Performance and health monitoring endpoints"),
            (name = "dashboard", description = "Dashboard metrics and analytics endpoints")
        )
    )]
    struct ApiDoc;

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/status", get(get_system_status))
        .route("/api/profile", post(trigger_profile_collection))
        .route("/api/config", get(handle_get_config))
        .route("/api/config/reload", post(handle_reload))
        .with_state(state);
        .route("/", get(|| async { "Crucible Backend API" }))
        .route("/.well-known/stellar.toml", get(stellar::get_stellar_toml))
        .nest(
            "/api/v1/profiling",
            Router::new()
                .route("/metrics", get(profiling::get_metrics))
                .route("/health", get(profiling::get_health))
                .route("/prometheus", get(profiling::get_prometheus_metrics)),
        )
        .nest("/api/v1/dashboard", Router::new()
            .route("/metrics", get(dashboard::get_dashboard_metrics))
            .route("/contracts/:contract_id/stats", get(dashboard::get_contract_stats))
            .with_state(dashboard_state)
        )
        .route("/api/status", get(profiling::get_system_status))
        .route("/api/profile", post(profiling::trigger_profile_collection))
        .with_state(profiling_state)
        .route("/api/dashboard", get(get_dashboard))
        .with_state(dashboard_state)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(middleware::from_fn_with_state(state.clone(), logging_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    tracing::info!("listening on {}", addr);

    tracing::info!("Crucible backend listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tokio::select! {
        res = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()) => {
            res?;
        },
        _ = worker.run() => {
            tracing::info!("Worker stopped");
        }
    }

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, starting graceful shutdown");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM, starting graceful shutdown");
        },
    }
}
