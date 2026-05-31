use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api::contracts::ApiResponse;
use crate::error::AppError;
use crate::services::compilation::{CompilationResult, CompilationService};
use crate::services::dependency_analyzer::{DependencyAnalysis, DependencyAnalyzer};
use crate::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileRequest {
    pub project_name: String,
    pub source_code: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeRequest {
    pub cargo_toml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    pub id: String,
    pub name: String,
    pub rpc_url: String,
    pub passphrase: String,
    pub status: String,
    pub ping_ms: u32,
    pub latest_ledger: u32,
    pub active_contracts_count: u32,
}

/// POST /api/v1/contracts/compile
pub async fn compile_contract(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CompileRequest>,
) -> Result<impl IntoResponse, AppError> {
    let db = state
        .db
        .clone()
        .ok_or_else(|| AppError::InternalError("Database connection not configured".to_string()))?;

    let service = CompilationService::new(db);
    let result = service
        .compile(&payload.project_name, &payload.source_code)
        .await
        .map_err(AppError::Database)?;

    Ok(Json(ApiResponse::new(result)))
}

/// POST /api/v1/contracts/analyze-dependencies
pub async fn analyze_dependencies(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AnalyzeRequest>,
) -> Result<impl IntoResponse, AppError> {
    let db = state
        .db
        .clone()
        .ok_or_else(|| AppError::InternalError("Database connection not configured".to_string()))?;

    let service = DependencyAnalyzer::new(db);
    let result = service
        .analyze(&payload.cargo_toml)
        .await
        .map_err(AppError::Database)?;

    Ok(Json(ApiResponse::new(result)))
}

/// GET /api/v1/networks
pub async fn get_networks() -> Result<impl IntoResponse, AppError> {
    let networks = vec![
        NetworkConfig {
            id: "mainnet".to_string(),
            name: "Soroban Mainnet".to_string(),
            rpc_url: "https://soroban-testnet.stellar.org:443".to_string(), // placeholder/mock url
            passphrase: "Public Global Stellar Network ; October 2015".to_string(),
            status: "online".to_string(),
            ping_ms: 82,
            latest_ledger: 1045231,
            active_contracts_count: 1420,
        },
        NetworkConfig {
            id: "testnet".to_string(),
            name: "Soroban Testnet".to_string(),
            rpc_url: "https://soroban-testnet.stellar.org".to_string(),
            passphrase: "Test SDF Network ; September 2015".to_string(),
            status: "online".to_string(),
            ping_ms: 34,
            latest_ledger: 452934,
            active_contracts_count: 328,
        },
        NetworkConfig {
            id: "futurenet".to_string(),
            name: "Soroban Futurenet".to_string(),
            rpc_url: "https://rpc-futurenet.stellar.org".to_string(),
            passphrase: "Test SDF Future Network ; October 2022".to_string(),
            status: "online".to_string(),
            ping_ms: 56,
            latest_ledger: 98124,
            active_contracts_count: 94,
        },
        NetworkConfig {
            id: "sandbox".to_string(),
            name: "Local Sandbox".to_string(),
            rpc_url: "http://localhost:8000".to_string(),
            passphrase: "Standalone Network ; Standalone".to_string(),
            status: "online".to_string(),
            ping_ms: 2,
            latest_ledger: 4239,
            active_contracts_count: 15,
        },
    ];

    Ok(Json(ApiResponse::new(networks)))
}
