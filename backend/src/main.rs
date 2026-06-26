use std::net::SocketAddr;
use std::sync::Arc;

use apalis::prelude::*;
use apalis_redis::RedisStorage;
use backend::{
    app_state::{build_application_states, SharedServices},
    config::{reload::ConfigManager, AppConfig, Environment},
    jobs::{monitor_transaction, TransactionMonitorJob},
    router::build_router,
    services::{
        contract_benchmark::ContractBenchmarkService,
        error_recovery::ErrorManager,
        log_aggregator::LogAggregator,
        log_alerts::AlertManager,
        sandbox::ContractSandboxService,
        sys_metrics::MetricsExporter,
        tracing::{TracingConfig, TracingService},
    },
};
use redis::{aio::ConnectionManager, Client as RedisClient};
use tokio::signal;
use tracing::info_span;

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
    let _enter = info_span!("app.startup").entered();

    let db_pool = config
        .database
        .to_sqlx_pool_options()
        .connect(&config.database.url)
        .await?;
    tracing::info!("Database connection established");

    let redis_client = RedisClient::open(config.redis.url.clone())?;

    let metrics_exporter = Arc::new(MetricsExporter::new());
    let error_manager = Arc::new(ErrorManager::new());
    let alert_manager = Arc::new(AlertManager::new());
    let (log_aggregator, log_receiver) = LogAggregator::new();
    let log_aggregator = Arc::new(log_aggregator);
    let sandbox_service = Arc::new(ContractSandboxService::default());
    let contract_benchmark_service = Arc::new(ContractBenchmarkService::new());
    let config_manager = Arc::new(ConfigManager::new(config.clone()));

    tokio::spawn(MetricsExporter::run_collector(metrics_exporter.clone()));
    tokio::spawn(LogAggregator::run_worker(log_receiver));

    let conn = ConnectionManager::new(redis_client.clone()).await?;
    let storage: RedisStorage<TransactionMonitorJob> = RedisStorage::new(conn);
    tracing::info!("Redis connection established");

    let worker = WorkerBuilder::new("monitor-worker")
        .backend(storage)
        .build_fn(monitor_transaction);

    let health_cache = ConnectionManager::new(redis_client.clone()).await?;
    let health_queue = ConnectionManager::new(redis_client.clone()).await?;

    let health_state = health::HealthState {
        db: db_pool.clone(),
        cache: health_cache,
        queue: health_queue,
    };

    let shared_services = SharedServices {
        metrics_exporter,
        error_manager,
        alert_manager,
        log_aggregator,
        contract_benchmark_service,
        config_manager: config_manager.clone(),
    };

    let states = build_application_states(db_pool.clone(), redis_client.clone(), &shared_services);

    let app = build_router(
        states,
        config_manager,
        db_pool.clone(),
        redis_client.clone(),
        sandbox_service,
        &config,
    );

    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    tracing::info!("Crucible backend listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let result = tokio::select! {
        res = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()) => {
            db_pool.close().await;
            res
        },
        _ = worker.run() => Ok(()),
    };

    result?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
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
