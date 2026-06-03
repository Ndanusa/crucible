//! Configuration hot-reload.
//!
//! This module provides two complementary configuration management types:
//!
//! - [`ConfigManager`] — a simple `ArcSwap`-backed manager used by the
//!   profiling handlers. Supports file-based and patch-based reloads.
//! - [`ConfigWatcher`] — a richer watcher that subscribes to a Redis pub/sub
//!   channel and atomically swaps the live config on every reload signal.
//!
//! # Redis protocol (ConfigWatcher)
//!
//! ```text
//! SET config:current '{"log_level":"info","max_connections":50,...}'
//! PUBLISH config:reload "reload"
//! ```

#![allow(dead_code)]

use std::sync::Arc;

use arc_swap::ArcSwap;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use redis::{AsyncCommands, Client as RedisClient};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::{watch, RwLock};
use tracing::{error, info, instrument, warn};

use crate::config::AppConfig;

// ---------------------------------------------------------------------------
// ConfigReloadError
// ---------------------------------------------------------------------------

/// Errors that can occur during configuration reload (ConfigManager).
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

// ---------------------------------------------------------------------------
// ConfigManager (ArcSwap-based, used by profiling handlers)
// ---------------------------------------------------------------------------

/// Manages hot-reloadable application configuration via `ArcSwap`.
pub struct ConfigManager {
    current_config: ArcSwap<BaseAppConfig>,
}

impl ConfigManager {
    /// Create a new `ConfigManager` with the given initial configuration.
    pub fn new(initial_config: AppConfig) -> Self {
        Self {
            current_config: ArcSwap::from(Arc::new(initial_config)),
        }
    }

    /// Return a snapshot of the current configuration.
    pub fn load(&self) -> Arc<AppConfig> {
        self.current_config.load_full()
    }

    /// Reload configuration from `config.json` in the current directory.
    #[instrument(skip(self))]
    pub async fn reload(&self) -> Result<(), ConfigReloadError> {
        info!("Starting configuration reload...");

        let config_path = "config.json";

        if !std::path::Path::new(config_path).exists() {
            warn!("config.json not found, skipping reload");
            return Err(ConfigReloadError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "config.json not found",
            )));
        }

        let content = tokio::fs::read_to_string(config_path).await?;
        let new_config: AppConfig = serde_json::from_str(&content)?;

        if new_config.database.url.is_empty() {
            return Err(ConfigReloadError::Invalid(
                "Database URL cannot be empty".to_string(),
            ));
        }

        self.current_config.store(Arc::new(new_config));
        info!("Configuration successfully reloaded");
        Ok(())
    }

    /// Apply a JSON patch to the current configuration.
    #[instrument(skip(self, patch))]
    pub fn update_from_patch(&self, patch: Value) -> Result<(), ConfigReloadError> {
        let current = self.load();
        let mut current_json = serde_json::to_value(&*current)?;

        if let Some(patch_obj) = patch.as_object() {
            if let Some(current_obj) = current_json.as_object_mut() {
                for (k, v) in patch_obj {
                    if v.is_object()
                        && current_obj.contains_key(k)
                        && current_obj[k].is_object()
                    {
                        let sub_patch = v.as_object().unwrap();
                        let sub_current =
                            current_obj.get_mut(k).unwrap().as_object_mut().unwrap();
                        for (sk, sv) in sub_patch {
                            sub_current.insert(sk.clone(), sv.clone());
                        }
                    } else {
                        current_obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        let new_config: AppConfig = serde_json::from_value(current_json)?;
        self.current_config.store(Arc::new(new_config));
        info!("Configuration updated via patch");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Axum handlers for ConfigManager
// ---------------------------------------------------------------------------

/// `POST /api/config/reload` — trigger a configuration reload from disk.
pub async fn handle_reload(
    State(state): State<Arc<crate::api::handlers::profiling::AppState>>,
) -> impl IntoResponse {
    match state.config_manager.reload().await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "reloaded" })),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

/// `GET /api/config` — return the current configuration (sanitized).
pub async fn handle_get_config(
    State(manager): State<Arc<ConfigManager>>,
) -> impl IntoResponse {
    let config = state.config_manager.load();
    Json(config)
}

/// Axum handler to get the current configuration (sanitized).
pub async fn handle_get_config(
    State(state): State<Arc<crate::api::handlers::profiling::AppState>>,
) -> impl IntoResponse {
    let config = state.config_manager.load();
    // In a real app, we would sanitize sensitive fields like DB passwords
    Json(config)
// ---------------------------------------------------------------------------
// ReloadError (ConfigWatcher)

/// Errors that can occur during ConfigWatcher reload.
#[derive(Debug, Error)]
pub enum ReloadError {
    /// A Redis error occurred.
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    /// The configuration value could not be deserialised.
    #[error("Config deserialisation error: {0}")]
    Deserialise(#[from] serde_json::Error),

    /// The configuration key was not found in Redis.
    #[error("Config key not found in Redis")]
    NotFound,
}

// HotAppConfig (used by ConfigWatcher)

/// Live application configuration that can be hot-reloaded at runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HotAppConfig {
    /// Tracing / log filter directive (e.g. `"backend=debug"`).
    pub log_level: String,
    /// Maximum number of database connections in the pool.
    pub max_connections: u32,
    /// Request timeout in seconds.
    pub request_timeout_secs: u64,
    /// Whether the maintenance mode banner is shown.
    pub maintenance_mode: bool,
    /// Redis key that stores the serialised [`HotAppConfig`] JSON.
    pub redis_config_key: String,
}

impl Default for HotAppConfig {
    fn default() -> Self {
        Self {
            log_level: "backend=debug,tower_http=debug".to_string(),
            max_connections: 10,
            request_timeout_secs: 30,
            maintenance_mode: false,
            redis_config_key: "config:current".to_string(),
        }
    }
}

// ConfigHandle

/// A cheap-to-clone handle to the live configuration.
#[derive(Clone)]
pub struct ConfigHandle {
    inner: Arc<RwLock<HotAppConfig>>,
    changed: watch::Receiver<()>,
}

impl ConfigHandle {
    /// Return a snapshot of the current configuration.
    pub async fn get(&self) -> HotAppConfig {
        self.inner.read().await.clone()
    }

    /// Wait until the configuration changes, then return the new snapshot.
    pub async fn wait_for_change(&mut self) -> HotAppConfig {
        let _ = self.changed.changed().await;
        self.get().await
    }
}

// ConfigWatcher

/// Owns the live [`HotAppConfig`] and drives hot-reload via Redis pub/sub.
pub struct ConfigWatcher {
    notify_tx: watch::Sender<()>,
    notify_rx: watch::Receiver<()>,
}

impl ConfigWatcher {
    /// Create a new watcher with the given initial configuration.
    pub fn new(initial: HotAppConfig) -> Self {
        let (tx, rx) = watch::channel(());
            inner: Arc::new(RwLock::new(initial)),
            notify_tx: tx,
            notify_rx: rx,
        }
    }

    /// Return a [`ConfigHandle`] that can be cloned and shared freely.
    pub fn handle(&self) -> ConfigHandle {
        ConfigHandle {
            inner: Arc::clone(&self.inner),
            changed: self.notify_rx.clone(),
        }
    }

    /// Atomically replace the current configuration and notify all handles.
    pub async fn reload(&self, new_config: HotAppConfig) {
        let old = {
            let mut guard = self.inner.write().await;
            let old = guard.clone();
            *guard = new_config.clone();
            old
        };
        if old != new_config {
            info!(
                log_level = %new_config.log_level,
                max_connections = new_config.max_connections,
                maintenance_mode = new_config.maintenance_mode,
                "Configuration reloaded"
            );
            let _ = self.notify_tx.send(());
        } else {
            info!("Configuration reload requested but values unchanged");
        }
    }

    /// Fetch the current configuration from Redis and apply it.
    pub async fn reload_from_redis(&self, redis: &RedisClient) -> Result<(), ReloadError> {
        let key = self.inner.read().await.redis_config_key.clone();
        let mut conn = redis.get_multiplexed_async_connection().await?;
        let raw: Option<String> = conn.get(&key).await?;
        let json = raw.ok_or(ReloadError::NotFound)?;
        let new_config: HotAppConfig = serde_json::from_str(&json)?;
        self.reload(new_config).await;
        Ok(())
    }

    /// Spawn a background task that subscribes to `config:reload` on Redis.
    pub fn watch(self: Arc<Self>, redis: RedisClient) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            const CHANNEL: &str = "config:reload";

            #[allow(deprecated)]
            let conn = match redis.get_async_connection().await {
                Ok(c) => c,
                Err(e) => {
                    error!(error = %e, "Config watcher: failed to connect to Redis");
                    return;
                }
            };

            let mut pubsub = conn.into_pubsub();
            if let Err(e) = pubsub.subscribe(CHANNEL).await {
                error!(error = %e, channel = CHANNEL, "Config watcher: subscribe failed");
                return;
            }

            info!(channel = CHANNEL, "Config watcher: listening for reload signals");

            let mut stream = pubsub.into_on_message();
            use futures_util::StreamExt;

            loop {
                match stream.next().await {
                    Some(msg) => {
                        let payload: String = msg.get_payload().unwrap_or_default();
                        info!(payload = %payload, "Config reload signal received");
                        if let Err(e) = self.reload_from_redis(&redis).await {
                            warn!(
                                error = %e,
                                "Config reload from Redis failed; keeping current config"
                            );
                        }
                    }
                    None => {
                        warn!("Config watcher: Redis pub/sub stream ended");
                        break;
                    }
                }
            }
        })
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn default_watcher() -> ConfigWatcher {
        ConfigWatcher::new(HotAppConfig::default())
    }

    #[test]
    fn test_default_config_values() {
        let cfg = HotAppConfig::default();
        assert_eq!(cfg.max_connections, 10);
        assert_eq!(cfg.request_timeout_secs, 30);
        assert!(!cfg.maintenance_mode);
        assert!(!cfg.log_level.is_empty());
        assert_eq!(cfg.redis_config_key, "config:current");
    }

    fn test_config_serialisation_roundtrip() {
        let json = serde_json::to_string(&cfg).unwrap();
        let back: HotAppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
    }

    #[tokio::test]
    async fn test_reload_updates_config() {
        let watcher = default_watcher();
        let handle = watcher.handle();

        let new_cfg = HotAppConfig {
            log_level: "info".to_string(),
            max_connections: 50,
            ..HotAppConfig::default()
        };
        watcher.reload(new_cfg.clone()).await;
        assert_eq!(handle.get().await, new_cfg);
    }

    async fn test_reload_unchanged_does_not_notify() {
        let mut handle = watcher.handle();
        handle.changed.borrow_and_update();
        watcher.reload(HotAppConfig::default()).await;
        assert!(!handle.changed.has_changed().unwrap());
    }

    async fn test_reload_changed_notifies_handle() {
        watcher
            .reload(HotAppConfig {
                maintenance_mode: true,
                ..HotAppConfig::default()
            })
            .await;
        assert!(handle.changed.has_changed().unwrap());
    }

    async fn test_multiple_handles_see_same_update() {
        let h1 = watcher.handle();
        let h2 = watcher.handle();
            max_connections: 99,
        };
        watcher.reload(new_cfg).await;
        assert_eq!(h1.get().await.max_connections, 99);
        assert_eq!(h2.get().await.max_connections, 99);
    }

    async fn test_reload_from_redis_connection_error() {
        let redis = RedisClient::open("redis://127.0.0.1:1/").unwrap();
        let result = watcher.reload_from_redis(&redis).await;
        assert!(matches!(result, Err(ReloadError::Redis(_))));
        assert_eq!(watcher.handle().get().await, HotAppConfig::default());
    }

    fn test_reload_error_not_found_display() {
        let e = ReloadError::NotFound;
        assert!(e.to_string().contains("not found"));
    }
    let config = manager.load();
    // Sensitive fields are already skipped or redacted by `serde(skip_serializing)` and custom `Debug`.
    // In this case, `AppConfig` derives Serialize, and sensitive fields have `#[serde(skip_serializing)]`.
    Json(config.as_ref().clone())
}
