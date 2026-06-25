use std::{net::SocketAddr, sync::Arc};

use apalis::prelude::*;
use apalis_redis::RedisStorage;
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use backend::{
    api::{
        handlers::{
            contracts, coverage, dashboard, errors, profiling, sandbox, stellar,
            ws::{ws_dashboard_handler, WsState},
        },
        middleware::logging::logging_middleware,
    },
    config::{
        reload::{handle_get_config, handle_reload, ConfigManager},
        AppConfig, Environment,
    },
    jobs::{monitor_transaction, TransactionMonitorJob},
    services::{
        audit::{self, AuditService},
        contract_benchmark::ContractBenchmarkService,
        error_recovery::ErrorManager,
        log_aggregator::LogAggregator,
        log_alerts::AlertManager,
        sandbox::ContractSandboxService,
        sys_metrics::MetricsExporter,
        test_coverage::TestCoverageService,
        tracing::TracingConfig,
        tracing::TracingService,
    },
};
use redis::aio::ConnectionManager;
use tokio::signal;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info_span;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        profiling::get_metrics,
        profiling::get_health,
        dashboard::get_dashboard_metrics,
        dashboard::get_contract_stats,
        audit::list_audit_reports,
        audit::get_audit_report,
    ),
    components(
        schemas(
            profiling::MetricsReport,
            profiling::HealthResponse,
            dashboard::DashboardMetrics,
            dashboard::ContractStats,
            audit::AuditEventRecord,
            audit::AuditEventRequest,
        )
    ),
    tags(
        (name = "profiling", description = "Performance and health monitoring endpoints"),
        (name = "dashboard", description = "Dashboard metrics and analytics endpoints"),
        (name = "audit", description = "Audit log endpoints"),
    )
)]
struct ApiDoc;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let env = Environment::from_env();
    let config = AppConfig::load(env).expect("Failed to load configuration");

    let tracing_config = TracingConfig::new(
        "crucible-backend".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    )
    .with_environment(env.as_str().to_string())
    .with_otlp_endpoint(
        config
            .observability
            .tracing_endpoint
            .clone()
            .unwrap_or_else(|| "http://localhost:4318/v1/traces".to_string()),
    );

    let _tracing_guard = TracingService::init(tracing_config)?;

    let startup_span = info_span!("app.startup");
    let _startup_enter = startup_span.enter();

    let db_span = TracingService::db_query_span("CONNECT postgresql", "postgres", "CONNECT");
    let _db_enter = db_span.enter();
    let db_pool = config
        .database
        .to_sqlx_pool_options()
        .connect(&config.database.url)
        .await?;
    tracing::info!("Database connection established");
    drop(_db_enter);

    let redis_client = redis::Client::open(config.redis_url.clone())?;
    let redis_span = TracingService::redis_command_span("CONNECT", None);
    let _redis_enter = redis_span.enter();
    let redis_conn_dashboard = ConnectionManager::new(redis_client.clone()).await?;
    let queue_conn = ConnectionManager::new(redis_client.clone()).await?;
    tracing::info!("Redis connection established");
    drop(_redis_enter);

    let metrics_exporter = Arc::new(MetricsExporter::new());
    let error_manager = Arc::new(ErrorManager::new());
    let alert_manager = Arc::new(AlertManager::new());
    let (log_aggregator, log_receiver) = LogAggregator::new();
    let log_aggregator = Arc::new(log_aggregator);
    let sandbox_service = Arc::new(ContractSandboxService::default());
    let contract_benchmark_service = Arc::new(ContractBenchmarkService::new());
    let config_manager = Arc::new(ConfigManager::new(AppConfig::default()));

    tokio::spawn(MetricsExporter::run_collector(metrics_exporter.clone()));
    tokio::spawn(LogAggregator::run_worker(log_receiver));

    let coverage_state = Arc::new(coverage::CoverageState {
        service: TestCoverageService::new(db_pool.clone(), redis_client.clone()),
    });

    let profiling_state = Arc::new(profiling::AppState {
        db: Some(db_pool.clone()),
        metrics_exporter: metrics_exporter.clone(),
        error_manager: error_manager.clone(),
        config_manager: config_manager.clone(),
        log_aggregator: log_aggregator.clone(),
        contract_benchmark_service: contract_benchmark_service.clone(),
        redis: redis_client.clone(),
    });

    let dashboard_state = Arc::new(dashboard::DashboardState {
        db: db_pool.clone(),
        redis_conn: redis_conn_dashboard.clone(),
        metrics_exporter: metrics_exporter.clone(),
        error_manager: error_manager.clone(),
        alert_manager: alert_manager.clone(),
        redis_client: redis_client.clone(),
    });

    let audit_service = Arc::new(AuditService::new(
        db_pool.clone(),
        Arc::new(redis_client.clone()),
    ));
    let ws_state = WsState {
        metrics_exporter: metrics_exporter.clone(),
        error_manager: error_manager.clone(),
    };

    let config_router = Router::new()
        .route("/api/config", get(handle_get_config))
        .route("/api/config/reload", post(handle_reload))
        .with_state(config_manager);

    let profiling_router = Router::new()
        .route("/metrics", get(profiling::get_metrics))
        .route("/health", get(profiling::get_health))
        .route("/prometheus", get(profiling::get_prometheus_metrics))
        .route("/status", get(profiling::get_system_status))
        .route("/profile", post(profiling::trigger_profile_collection))
        .route(
            "/contracts/benchmark",
            post(profiling::run_contract_benchmark),
        )
        .with_state(profiling_state.clone());

    let dashboard_router = Router::new()
        .route("/", get(dashboard::get_dashboard))
        .route("/metrics", get(dashboard::get_dashboard_metrics))
        .route(
            "/contracts/:contract_id/stats",
            get(dashboard::get_contract_stats),
        )
        .with_state(dashboard_state.clone());

    let contracts_router = Router::new()
        .route("/compile", post(contracts::compile_contract))
        .route(
            "/analyze-dependencies",
            post(contracts::analyze_dependencies),
        )
        .route("/compliance-check", post(contracts::check_compliance))
        .route(
            "/logs",
            post(contracts::log_contract_call).get(contracts::get_contract_logs),
        )
        .route("/upgrade-plan", post(contracts::create_upgrade_plan))
        .route("/templates", get(contracts::get_templates));

    let coverage_router = Router::new()
        .route("/", post(coverage::submit_coverage))
        .route("/:project", get(coverage::get_latest_coverage))
        .with_state(coverage_state);

    let admin_router = Router::new()
        .route(
            "/system-stats",
            get(backend::api::handlers::admin::get_system_stats),
        )
        .route(
            "/maintenance",
            post(backend::api::handlers::admin::set_maintenance_mode),
        )
        .route("/logs", get(backend::api::handlers::admin::get_admin_logs));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(|| async { "Crucible Backend API" }))
        .route("/.well-known/stellar.toml", get(stellar::get_stellar_toml))
        .merge(config_router)
        .nest("/api/v1/profiling", profiling_router)
        .nest("/api/v1/dashboard", dashboard_router)
        .nest("/api/v1/audit", audit::routes(audit_service))
        .nest(
            "/api/v1/errors",
            errors::error_analytics_routes(db_pool.clone(), redis_client.clone()),
        )
        .nest("/api/v1/contracts", contracts_router)
        .route("/api/v1/networks", get(contracts::get_networks))
        .nest("/api/v1/admin", admin_router)
        .nest("/api/v1/sandbox", sandbox::routes(sandbox_service))
        .nest("/api/v1/coverage", coverage_router)
        .route(
            "/api/v1/ws/dashboard",
            get(ws_dashboard_handler).with_state(Arc::new(ws_state)),
        )
        .route("/api/dashboard", get(dashboard::get_dashboard))
        .with_state(dashboard_state)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(middleware::from_fn_with_state(
            profiling_state,
            logging_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let storage: RedisStorage<TransactionMonitorJob> = RedisStorage::new(queue_conn);
    let worker = WorkerBuilder::new("monitor-worker")
        .backend(storage)
        .build_fn(monitor_transaction);

    let server = axum::serve(listener, app);
    let result = tokio::select! {
        res = server.with_graceful_shutdown(shutdown_signal()) => res,
        _ = worker.run() => {
            tracing::info!("Worker stopped");
            Ok(())
        }
    };

    if let Err(error) = &result {
        tracing::error!("Application error: {error}");
    }

    result?;
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
        _ = ctrl_c => tracing::info!("Received Ctrl+C, initiating graceful shutdown"),
        _ = terminate => tracing::info!("Received SIGTERM, initiating graceful shutdown"),
    }
}
