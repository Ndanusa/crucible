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
//! This module provides two complementary APIs:
//! ## [`ConfigManager`] — patch-based updates
//! Wraps [`AppConfig`] in an [`arc_swap::ArcSwap`] for lock-free reads.
//! Supports atomic replacement via [`ConfigManager::reload`] and partial
//! JSON-patch updates via [`ConfigManager::update_from_patch`].
//! ## [`ConfigWatcher`] — Redis pub/sub driven reload
//! Subscribes to the `config:reload` Redis channel. On every message it
//! fetches the JSON stored at `config:current`, deserialises it, and
//! atomically swaps the in-memory value. All readers that hold a
//! [`ConfigHandle`] see the new values on their next read.
//! # Axum handlers
//! | Route | Handler | Description |
//! |---|---|---|
//! | `GET /api/config` | [`handle_get_config`] | Return current config as JSON |
//! | `POST /api/config/reload` | [`handle_reload`] | Reload config from `config.json` |
//! # Redis protocol
//!
//! ```text
//! SET config:current '{"log_level":"info","max_connections":50,...}'
//! PUBLISH config:reload "reload"
//! ```

#![allow(dead_code)]
//!
//! # Example
//! ```rust,no_run
//! use std::sync::Arc;
//! use backend::config::{AppConfig, reload::ConfigWatcher};
//! # async fn example() {
//! let watcher = Arc::new(ConfigWatcher::new(AppConfig::default()));
//! let handle = watcher.handle();
//! // Read the current config
//! let cfg = handle.get().await;
//! println!("log level: {}", cfg.log_level);
//! // Trigger a manual reload
//! watcher.reload(AppConfig { maintenance_mode: true, ..AppConfig::default() }).await;
//! # }
//! ```

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
// Error type
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

    /// An I/O error occurred (e.g. reading config.json).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The configuration value was semantically invalid.
    #[error("Invalid configuration: {0}")]
    Invalid(String),
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
impl IntoResponse for ReloadError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            ReloadError::Invalid(_) | ReloadError::Deserialise(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(serde_json::json!({ "error": self.to_string() }))).into_response()
    }
}

// ---------------------------------------------------------------------------
// ConfigManager — ArcSwap-based, patch-capable

/// Manages hot-reloadable application configuration via lock-free reads.
///
/// Wrap in an [`Arc`] and share across Axum handlers via application state.
pub struct ConfigManager {
    current: ArcSwap<AppConfig>,
}

impl ConfigManager {
    /// Create a new manager with the given initial configuration.
    pub fn new(initial: AppConfig) -> Self {
            current: ArcSwap::from(Arc::new(initial)),
        }
    }

    /// Return a snapshot of the current configuration.
    ///
    /// This is a lock-free read — safe to call from hot paths.
    pub fn load(&self) -> Arc<AppConfig> {
        self.current.load_full()
    }

    /// Atomically replace the current configuration.
    /// Reads the JSON value from `config.json` in the current directory,
    /// validates it, and swaps it in.
    #[instrument(skip(self))]
    pub async fn reload(&self) -> Result<(), ReloadError> {
        info!("Starting configuration reload from config.json");

        let path = "config.json";
        if !std::path::Path::new(path).exists() {
            warn!("config.json not found, aborting reload");
            return Err(ReloadError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "config.json not found",
            )));
        }

        let content = tokio::fs::read_to_string(path).await?;
        let new_config: AppConfig = serde_json::from_str(&content)?;

        if new_config.database.url.is_empty() {
            return Err(ReloadError::Invalid("database.url cannot be empty".into()));
        }

        self.current.store(Arc::new(new_config));
        info!("Configuration reloaded successfully");
        Ok(())
    }

    /// Apply a partial JSON patch to the current configuration.
    /// Top-level and one-level-deep object keys are merged; all other values
    /// are replaced. Returns an error if the result cannot be deserialised
    /// into [`AppConfig`].
    #[instrument(skip(self, patch))]
    pub fn update_from_patch(&self, patch: Value) -> Result<(), ReloadError> {
        let current = self.load();
        let mut current_json = serde_json::to_value(&*current)?;

        if let (Some(patch_obj), Some(current_obj)) =
            (patch.as_object(), current_json.as_object_mut())
        {
            for (k, v) in patch_obj {
                if v.is_object() {
                    if let Some(sub) = current_obj.get_mut(k).and_then(|s| s.as_object_mut()) {
                        for (sk, sv) in v.as_object().unwrap() {
                            sub.insert(sk.clone(), sv.clone());
                        }
                        continue;
                    }
                }
                current_obj.insert(k.clone(), v.clone());
            }
        }

        let new_config: AppConfig = serde_json::from_value(current_json)?;
        info!("Configuration updated via patch");
    }
}

// ConfigHandle — cheap clone, shared reader with change notification

/// A cheap-to-clone handle to the live configuration.
#[derive(Clone)]
pub struct ConfigHandle {
    inner: Arc<RwLock<HotAppConfig>>,
    inner: Arc<RwLock<AppConfig>>,
    changed: watch::Receiver<()>,
}

impl ConfigHandle {
    /// Return a snapshot of the current configuration.
    pub async fn get(&self) -> HotAppConfig {
        self.inner.read().await.clone()
    }

    /// Wait until the configuration changes, then return the new snapshot.
    pub async fn wait_for_change(&mut self) -> HotAppConfig {
    pub async fn wait_for_change(&mut self) -> AppConfig {
        let _ = self.changed.changed().await;
        self.get().await
    }
}

// ConfigWatcher

/// Owns the live [`HotAppConfig`] and drives hot-reload via Redis pub/sub.
// ---------------------------------------------------------------------------
// ConfigWatcher — Redis pub/sub driven reload

/// Owns the live [`AppConfig`] and drives hot-reload via Redis pub/sub.
///
/// Wrap in an [`Arc`] to share across tasks.
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
    ///
    /// If the new config is identical to the current one, no notification is
    /// sent.
    pub async fn reload(&self, new_config: AppConfig) {
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
    ///
    /// Reads the JSON value stored at the key `config:current`, deserialises
    /// it, and calls [`Self::reload`].
    /// # Errors
    /// Returns [`ReloadError`] if the Redis key is absent, the connection
    /// fails, or the JSON cannot be deserialised.
    pub async fn reload_from_redis(&self, redis: &RedisClient) -> Result<(), ReloadError> {
        const KEY: &str = "config:current";
        let mut conn = redis.get_multiplexed_async_connection().await?;
        let raw: Option<String> = conn.get(KEY).await?;
        let json = raw.ok_or(ReloadError::NotFound)?;
        let new_config: HotAppConfig = serde_json::from_str(&json)?;
        self.reload(new_config).await;
        Ok(())
    }

    /// Spawn a background task that subscribes to `config:reload` on Redis.
    /// Spawn a background task that subscribes to `config:reload` on Redis
    /// and calls [`Self::reload_from_redis`] on every message.
    ///
    /// The task runs until the Redis pub/sub stream ends or the process exits.
    /// Connection errors are logged and the task exits — callers may restart
    /// it if desired.
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

            use futures_util::StreamExt;
            let mut stream = pubsub.into_on_message();

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

// ---------------------------------------------------------------------------
// Axum handlers

/// `POST /api/config/reload` — Reload configuration from `config.json`.
///
/// Returns `200 OK` on success or an error response if the file is missing
/// or the JSON is invalid.
pub async fn handle_reload(
    State(state): State<Arc<crate::api::handlers::profiling::AppState>>,
) -> Result<impl IntoResponse, ReloadError> {
    state.config_manager.reload().await?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "status": "reloaded" }))))
}

/// `GET /api/config` — Return the current configuration as JSON.
/// Sensitive fields (e.g. database passwords embedded in URLs) are returned
/// as-is; callers should restrict access to this endpoint appropriately.
pub async fn handle_get_config(
) -> impl IntoResponse {
    let config = state.config_manager.load();
    Json(config)
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    // --- ConfigManager ---

    #[tokio::test]
    async fn test_manager_load_returns_initial() {
        let mgr = ConfigManager::new(AppConfig::default());
        assert_eq!(*mgr.load(), AppConfig::default());
    }

    #[tokio::test]
    async fn test_manager_reload_missing_file() {
        let mgr = ConfigManager::new(AppConfig::default());
        let result = mgr.reload().await;
        assert!(matches!(result, Err(ReloadError::Io(_))));
        // Config must be unchanged.
        assert_eq!(*mgr.load(), AppConfig::default());
    }

    #[test]
    fn test_manager_patch_top_level_field() {
        let mgr = ConfigManager::new(AppConfig::default());
        mgr.update_from_patch(serde_json::json!({ "log_level": "warn" }))
            .unwrap();
        assert_eq!(mgr.load().log_level, "warn");
    }

    #[test]
    fn test_manager_patch_nested_field() {
        let mgr = ConfigManager::new(AppConfig::default());
        mgr.update_from_patch(serde_json::json!({ "server": { "port": 4000 } }))
            .unwrap();
        let cfg = mgr.load();
        assert_eq!(cfg.server.port, 4000);
        // Other nested fields preserved.
        assert_eq!(cfg.server.host, "0.0.0.0");
    }

    #[test]
    fn test_manager_patch_preserves_unpatched_fields() {
        let mgr = ConfigManager::new(AppConfig::default());
        mgr.update_from_patch(serde_json::json!({ "maintenance_mode": true }))
            .unwrap();
        let cfg = mgr.load();
        assert!(cfg.maintenance_mode);
        assert_eq!(cfg.max_connections, 10); // unchanged
    }

    // --- ConfigWatcher ---

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
    async fn test_watcher_reload_updates_config() {
        let watcher = default_watcher();
        let handle = watcher.handle();

        let new_cfg = HotAppConfig {
            log_level: "info".to_string(),
        let new_cfg = AppConfig {
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
    #[tokio::test]
    async fn test_watcher_reload_unchanged_no_notify() {
        let watcher = default_watcher();

        watcher.reload(AppConfig::default()).await;

    }

    async fn test_watcher_reload_changed_notifies() {

            .reload(AppConfig { maintenance_mode: true, ..AppConfig::default() })
            .await;
        assert!(handle.changed.has_changed().unwrap());
    }

    async fn test_multiple_handles_see_same_update() {
        let h1 = watcher.handle();
        let h2 = watcher.handle();
            max_connections: 99,
        };
        watcher.reload(new_cfg).await;
    #[tokio::test]
    async fn test_watcher_multiple_handles_see_update() {
        let watcher = default_watcher();

        watcher
            .reload(AppConfig { max_connections: 99, ..AppConfig::default() })
            .await;

        assert_eq!(h1.get().await.max_connections, 99);
        assert_eq!(h2.get().await.max_connections, 99);
    }

    async fn test_reload_from_redis_connection_error() {
        let redis = RedisClient::open("redis://127.0.0.1:1/").unwrap();
        let result = watcher.reload_from_redis(&redis).await;
        assert!(matches!(result, Err(ReloadError::Redis(_))));
        assert_eq!(watcher.handle().get().await, HotAppConfig::default());
    }

    #[tokio::test]
    async fn test_watcher_wait_for_change() {
        let watcher = Arc::new(default_watcher());
        let mut handle = watcher.handle();
        handle.changed.borrow_and_update();

        let w2 = Arc::clone(&watcher);
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            w2.reload(AppConfig { maintenance_mode: true, ..AppConfig::default() })
                .await;
        });

        let updated = handle.wait_for_change().await;
        assert!(updated.maintenance_mode);
    }

    async fn test_watcher_reload_from_redis_connection_error() {
        let watcher = default_watcher();
        // Port 1 is never open.
        assert_eq!(watcher.handle().get().await, AppConfig::default());
    }

    // --- ReloadError ---

    #[test]
    fn test_reload_error_not_found_display() {
        assert!(ReloadError::NotFound.to_string().contains("not found"));
    }

    #[test]
    fn test_reload_error_invalid_display() {
        let e = ReloadError::Invalid("bad value".into());
        assert!(e.to_string().contains("bad value"));
    }
    let config = manager.load();
    // Sensitive fields are already skipped or redacted by `serde(skip_serializing)` and custom `Debug`.
    // In this case, `AppConfig` derives Serialize, and sensitive fields have `#[serde(skip_serializing)]`.
    Json(config.as_ref().clone())

    #[test]
    fn test_reload_error_deserialise_display() {
        let inner = serde_json::from_str::<AppConfig>("not json").unwrap_err();
        let e = ReloadError::Deserialise(inner);
        assert!(!e.to_string().is_empty());
    }

    // --- AppConfig ---

    fn test_appconfig_default_values() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.max_connections, 10);
        assert_eq!(cfg.request_timeout_secs, 30);
        assert!(!cfg.maintenance_mode);
        assert!(!cfg.log_level.is_empty());
        assert_eq!(cfg.server.port, 3000);
        assert_eq!(cfg.server.host, "0.0.0.0");
    }

    fn test_appconfig_serialisation_roundtrip() {
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
    }
}
