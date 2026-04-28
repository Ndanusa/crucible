use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    #[error("Internal server error")]
    InternalServerError,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::DatabaseError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::RedisError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::InternalServerError => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
