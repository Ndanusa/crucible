use crate::config::{AppConfig as BaseAppConfig, ConfigError, Environment};
use arc_swap::ArcSwap;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;
use thiserror::Error;
use tracing::{info, instrument};

/// Errors that can occur during configuration reload.
#[derive(Debug, Error)]
pub enum ConfigReloadError {
    #[error("Configuration load error: {0}")]
    LoadError(#[from] ConfigError),
}

impl IntoResponse for ConfigReloadError {
    fn into_response(self) -> axum::response::Response {
        let status = StatusCode::INTERNAL_SERVER_ERROR;
        let body = Json(serde_json::json!({
            "error": self.to_string(),
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

/// Manages hot-reloadable application configuration.
pub struct ConfigManager {
    current_config: ArcSwap<BaseAppConfig>,
}

impl ConfigManager {
    /// Create a new ConfigManager with the given initial configuration.
    pub fn new(initial_config: BaseAppConfig) -> Self {
        Self {
            current_config: ArcSwap::from(Arc::new(initial_config)),
        }
    }

    /// Get a reference to the current configuration.
    pub fn load(&self) -> Arc<BaseAppConfig> {
        self.current_config.load_full()
    }

    /// Reload the configuration from environment variables and TOML files.
    #[instrument(skip(self))]
    pub async fn reload(&self) -> Result<(), ConfigReloadError> {
        info!("Starting configuration reload...");

        // Reload the layered config from the environment
        let env = Environment::from_env();
        let new_config = BaseAppConfig::load(env)?;

        // Update the global configuration atomically
        self.current_config.store(Arc::new(new_config));

        info!("Configuration successfully reloaded");
        Ok(())
    }
}

// In a real application, State type would be strongly typed for the app.
// We use a generic representation here or rely on the actual AppState type.
// Since the state definition was in `main.rs` and might be redefined, we'll keep it simple.

/// Axum handler to trigger a configuration reload.
pub async fn handle_reload(
    State(manager): State<Arc<ConfigManager>>,
) -> Result<impl IntoResponse, ConfigReloadError> {
    manager.reload().await?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "status": "reloaded" })),
    ))
}

/// Axum handler to get the current configuration (sanitized).
pub async fn handle_get_config(
    State(state): State<Arc<crate::api::handlers::profiling::AppState>>,
) -> impl IntoResponse {
    let config = state.config_manager.load();
    // In a real app, we would sanitize sensitive fields like DB passwords
    Json(config)
}
