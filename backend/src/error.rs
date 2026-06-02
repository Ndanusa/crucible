//! Custom error types for the Crucible backend.
//!
//! Provides a unified [`AppError`] type that maps internal errors into
//! appropriate HTTP status codes and JSON error responses.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::json;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// 500 — An internal Redis error occurred.
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    /// 500 — A serialization error occurred.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Internal server error: {0}")]
    Internal(String),

    /// 502 — Stellar network communication failure.
    #[error("Stellar operation failed: {0}")]
    StellarError(String),
}

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    /// Wrap a Redis error.
    pub fn redis(e: redis::RedisError) -> Self {
        AppError::Redis(e)
    }

    /// Wrap a serialization error.
    pub fn serialization(e: serde_json::Error) -> Self {
        AppError::Serialization(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Database(ref e) => {
                error!("Database error occurred: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "A database error occurred".to_string(),
                )
            }
            AppError::Redis(ref e) => {
                error!("Redis error occurred: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "A cache error occurred".to_string(),
                )
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized access".to_string()),
            AppError::StellarError(msg) => {
                error!("Stellar error: {}", msg);
                (
                    StatusCode::BAD_GATEWAY,
                    "Failed to communicate with Stellar network".to_string(),
                )
            }
            _ => {
                error!("Internal error: {:?}", self);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal server error occurred".to_string(),
                )
            }
        };

        let body = Json(json!({
            "error": message,
            "code": status.as_u16(),
        }));

        (status, body).into_response()
    }
}
