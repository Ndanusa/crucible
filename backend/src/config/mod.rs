//! Application configuration.
//!
//! - [`Config`] — environment-variable-based startup configuration.
//! - [`AppConfig`] — hot-reloadable runtime configuration.
//! - [`reload`] — [`ConfigManager`] and Axum handlers for live config updates.

pub mod reload;

use config::{Config, Environment as ConfigEnvironment, File, FileFormat};
use serde::{Deserialize, Serialize};
use std::env;

/// Environment-based application configuration.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
/// Startup configuration loaded from environment variables.
///
/// Read once at process start. For values that change at runtime without a
/// restart, see [`AppConfig`] and [`reload::ConfigManager`].
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub server_port: u16,
    pub environment: String,
    pub log_level: String,
//! CONFIG APPROACH: Option A — layered config crate
//! Rationale: Using the `config` crate provides a robust, layered approach where environment-specific
//! defaults are cleanly defined in TOML files, while sensitive secrets and infrastructure-specific
//! overrides are passed securely via environment variables. This prevents environment variable sprawl,
//! ensures typed nested structures, and makes local development frictionless without compromising
//! production security.

use std::str::FromStr;

pub mod database;
pub mod error;
pub mod observability;
pub mod redis;
pub mod server;

#[cfg(test)]
mod tests;

pub use database::DatabaseConfig;
pub use error::ConfigError;
pub use observability::ObservabilityConfig;
pub use redis::RedisConfig;
pub use server::ServerConfig;

/// The execution environment of the application.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl FromStr for Environment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Ok(Environment::Development),
            "staging" | "stg" => Ok(Environment::Staging),
            "production" | "prod" => Ok(Environment::Production),
            _ => Err(format!("Unknown environment: {}", s)),
        }
    }
}

impl Environment {
    /// Returns the string representation of the environment.
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Development => "development",
            Environment::Staging => "staging",
            Environment::Production => "production",
        }
    }

    /// Loads the environment from the `APP_ENV` environment variable.
    /// Defaults to `Development` if the variable is missing or unparseable.
    pub fn from_env() -> Self {
        match std::env::var("APP_ENV") {
            Ok(val) => match Environment::from_str(&val) {
                Ok(env) => env,
                Err(e) => {
                    tracing::warn!("Failed to parse APP_ENV: {}. Defaulting to Development.", e);
                    Environment::Development
                }
            },
            Err(_) => {
                tracing::warn!("APP_ENV environment variable not set. Defaulting to Development.");
                Environment::Development
            }
        }
    }
}

/// The root configuration object.
///
/// **Usage Pattern:**
/// `AppConfig::load` should be called exactly once during application startup.
/// The resulting configuration should be wrapped in an `std::sync::Arc<AppConfig>` and
/// shared across the application state to ensure consistent configuration reads without
/// cloning large structs or repeatedly parsing environments.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub observability: ObservabilityConfig,
}

// Temporary fallback for testing/reloading if missing in main.rs
impl Default for AppConfig {
    fn default() -> Self {
        Self::load(Environment::Development).expect("Failed to load default configuration")
    }
}

impl AppConfig {
    /// Loads the configuration by layering default settings, environment-specific
    /// settings, and finally environment variable overrides.
    pub fn load(env: Environment) -> Result<Self, ConfigError> {
        // Embed the TOML files into the binary to avoid distributing config files in production.
        let default_config = include_str!("defaults/default.toml");

        let env_config = match env {
            Environment::Development => include_str!("defaults/development.toml"),
            Environment::Staging => include_str!("defaults/staging.toml"),
            Environment::Production => include_str!("defaults/production.toml"),
        };

        let builder = Config::builder()
            // 1. Base configuration defaults
            .add_source(File::from_str(default_config, FileFormat::Toml))
            // 2. Environment-specific overrides
            .add_source(File::from_str(env_config, FileFormat::Toml))
            // 3. Environment variables (e.g., APP_DATABASE__URL overrides database.url)
            .add_source(
                ConfigEnvironment::with_prefix("APP")
                    .separator("__")
                    .ignore_empty(true),
            );

        let config_result = builder
            .build()
            .map_err(|e| ConfigError::LoadError(e.to_string()))?;
        let app_config: AppConfig = config_result
            .try_deserialize()
            .map_err(|e| ConfigError::LoadError(e.to_string()))?;

        app_config.validate(env)?;

        Ok(app_config)
    }

    /// Validates the loaded configuration according to the current environment's rules.
    fn validate(&self, env: Environment) -> Result<(), ConfigError> {
        let mut errors = Vec::new();

        if env == Environment::Production && self.server.tls.is_none() {
            errors.push(
                "TLS configuration is strictly required in the Production environment.".to_string(),
            );
        }
    }
}

/// Environment-based application configuration.
/// Simple environment-based config loader (used by main.rs).
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub server_port: u16,
    pub environment: String,
    pub log_level: String,
}

        Ok(())
impl Config {
    /// Loads configuration from environment variables.
    pub fn from_env() -> Result<Self, anyhow::Error> {
        dotenvy::dotenv().ok();
    }
}

        Ok(Config {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/backend".into()),
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into()),
            server_port: env::var("PORT")
                .unwrap_or_else(|_| "3000".into())
                .parse()?,
            environment: env::var("APP_ENV").unwrap_or_else(|_| "development".into()),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()),
        })
    /// Load configuration from environment variables (`.env` file optional).

            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".into()),
            environment: env::var("APP_ENV")
                .unwrap_or_else(|_| "development".into()),
            log_level: env::var("LOG_LEVEL")
                .unwrap_or_else(|_| "info".into()),
    }
}

// ---------------------------------------------------------------------------
// AppConfig — hot-reloadable runtime configuration

/// Server bind configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

/// Database pool configuration.
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

/// Redis connection configuration.
pub struct RedisConfig {
}

/// Live application configuration that can be hot-reloaded at runtime.
/// All fields have sensible defaults so the application starts without any
/// external configuration source. Use [`reload::ConfigManager`] to update
/// these values without restarting the process.
    /// Tracing / log filter directive (e.g. `"backend=debug"`).
    /// Maximum number of database connections in the pool.
    /// Request timeout in seconds.
    pub request_timeout_secs: u64,
    /// Whether the maintenance mode banner is shown.
    pub maintenance_mode: bool,
}

        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3000,
            database: DatabaseConfig {
                url: "postgres://postgres:postgres@localhost:5432/crucible".to_string(),
                max_connections: 10,
            redis: RedisConfig {
                url: "redis://127.0.0.1:6379".to_string(),
            log_level: "backend=debug,tower_http=debug".to_string(),
            max_connections: 10,
            request_timeout_secs: 30,
            maintenance_mode: false,
        }
    }
}
