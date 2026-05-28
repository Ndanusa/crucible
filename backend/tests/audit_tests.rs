use super::audit::*;
use axum::http::StatusCode;
use axum::Json;
use serde_json::json;
use sqlx::{PgPool, Executor};
use std::sync::Arc;
use redis::AsyncCommands;
use tokio::sync::OnceCell;

// Mock or test helpers for DB and Redis
static DB_POOL: OnceCell<PgPool> = OnceCell::const_new();
static REDIS_CLIENT: OnceCell<Arc<redis::Client>> = OnceCell::const_new();

async fn setup() -> (AuditService, PgPool, Arc<redis::Client>) {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set for tests");
    let db = PgPool::connect(&db_url).await.unwrap();
    let redis = Arc::new(redis::Client::open(redis_url).unwrap());
    (AuditService::new(db.clone(), redis.clone()), db, redis)
}

#[tokio::test]
async fn test_log_event_success() {
    let (service, db, redis) = setup().await;
    let event = AuditEvent {
        event_type: "login_attempt".to_string(),
        user_id: Some("user123".to_string()),
        details: json!({"ip": "127.0.0.1", "success": true}),
        timestamp: chrono::Utc::now(),
    };
    let result = service.log_event(event.clone()).await;
    assert!(result.is_ok());
    // Check DB
    let row = sqlx::query!("SELECT * FROM audit_logs WHERE event_type = $1 ORDER BY timestamp DESC LIMIT 1", event.event_type)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(row.user_id, Some("user123".to_string()));
    // Check Redis
    let mut conn = redis.get_async_connection().await.unwrap();
    let val: String = conn.lpop("audit_queue", None).await.unwrap();
    let parsed: AuditEvent = serde_json::from_str(&val).unwrap();
    assert_eq!(parsed.event_type, "login_attempt");
}

#[tokio::test]
async fn test_log_audit_event_handler() {
    let (service, _, _) = setup().await;
    let app = axum::Router::new().merge(routes(Arc::new(service)));
    let payload = AuditEventRequest {
        event_type: "password_reset".to_string(),
        user_id: Some("user456".to_string()),
        details: json!({"ip": "10.0.0.1", "success": false}),
    };
    let body = serde_json::to_vec(&payload).unwrap();
    let response = axum::http::Request::builder()
        .method("POST")
        .uri("/audit/log")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = axum::Server::bind(&"127.0.0.1:0".parse().unwrap())
        .serve(app.into_make_service())
        .with_graceful_shutdown(async { tokio::time::sleep(std::time::Duration::from_millis(100)).await })
        .await;
    assert!(resp.is_ok());
}
