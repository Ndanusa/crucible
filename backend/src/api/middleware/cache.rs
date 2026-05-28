use std::sync::Arc;
use axum::{
    http::{HeaderMap, HeaderValue, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Request,
};
use redis::{Client, AsyncCommands};
use tracing::{info, debug, warn, error};
use serde::{Serialize, Deserialize};
use std::time::Duration;

/// Cache key generator for HTTP requests
#[derive(Debug, Clone)]
pub struct CacheKeyGenerator;

impl CacheKeyGenerator {
    /// Generate a cache key from request
    pub fn generate_key(request: &Request) -> String {
        let method = request.method().as_str();
        let uri = request.uri().to_string();
        let query = request.uri().query().unwrap_or("");
        
        // Create hashable key
        format!("{}:{}:{}", method, uri, query)
    }
}

/// Response caching middleware
#[derive(Debug, Clone)]
pub struct CacheMiddleware {
    redis_client: Client,
    default_ttl: Duration,
}

impl CacheMiddleware {
    /// Create new cache middleware
    pub fn new(redis_client: Client, default_ttl: Duration) -> Self {
        Self {
            redis_client,
            default_ttl,
        }
    }

    /// Try to get cached response
    async fn get_cached_response(&self, key: &str) -> Result<Option<CachedResponse>, Box<dyn std::error::Error>> {
        let mut conn = self.redis_client.get_async_connection().await?;
        
        let cached: Option<String> = conn.get(key).await?;
        
        match cached {
            Some(json) => {
                let cached_response: CachedResponse = serde_json::from_str(&json)?;
                Ok(Some(cached_response))
            }
            None => Ok(None),
        }
    }

    /// Store response in cache
    async fn store_response(&self, key: &str, response: &Response) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = self.redis_client.get_async_connection().await?;
        
        // Convert response to cacheable format
        let cached_response = CachedResponse::from_response(response);
        
        let json = serde_json::to_string(&cached_response)?;
        
        // Store with TTL
        conn.set_ex(key, json, self.default_ttl.as_secs() as usize).await?;
        
        Ok(())
    }
}

/// Cached response structure
#[derive(Serialize, Deserialize, Debug)]
pub struct CachedResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl CachedResponse {
    /// Create cached response from HTTP response
    pub fn from_response(response: &Response) -> Self {
        let (parts, body) = response.into_parts();
        
        let headers: Vec<(String, String)> = parts
            .headers
            .iter()
            .map(|(name, value)| {
                (
                    name.to_string(),
                    value.to_str().unwrap_or("").to_string(),
                )
            })
            .collect();
        
        Self {
            status: parts.status.as_u16(),
            headers,
            body: body.into_bytes(),
        }
    }

    /// Convert back to HTTP response
    pub fn into_response(self) -> Response {
        let mut response = Response::builder()
            .status(StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR));
        
        // Add headers
        for (name, value) in self.headers {
            if let Ok(header_name) = name.parse() {
                if let Ok(header_value) = HeaderValue::from_str(&value) {
                    response = response.header(header_name, header_value);
                }
            }
        }
        
        response.body(axum::body::Body::from(self.body)).unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(axum::body::Body::empty())
                .unwrap()
        })
    }
}

/// Axum middleware function
pub async fn cache_middleware(
    request: Request,
    next: Next,
) -> Response {
    // This is a placeholder - in real implementation, this would be configured
    // with the actual Redis client and TTL
    let key = CacheKeyGenerator::generate_key(&request);
    
    // For now, just pass through to next handler
    // In production, this would check cache first, then call next if not found
    next.run(request).await
}

/// Cache middleware layer
#[derive(Debug, Clone)]
pub struct CacheLayer {
    middleware: CacheMiddleware,
}

impl CacheLayer {
    pub fn new(redis_client: Client, default_ttl: Duration) -> Self {
        Self {
            middleware: CacheMiddleware::new(redis_client, default_ttl),
        }
    }
}

// Implement tower::Layer trait
impl<S> tower::Layer<S> for CacheLayer {
    type Service = CacheService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CacheService {
            inner,
            middleware: self.middleware.clone(),
        }
    }
}

/// Cache service that wraps the inner service
#[derive(Debug, Clone)]
pub struct CacheService<S> {
    inner: S,
    middleware: CacheMiddleware,
}

impl<S, Req> tower::Service<Req> for CacheService<S>
where
    S: tower::Service<Req> + Clone + Send + 'static,
    S::Future: Send + 'static,
    Req: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let mut inner = self.inner.clone();
        let middleware = self.middleware.clone();
        
        Box::pin(async move {
            // In production, this would check cache first
            // For now, just call inner service
            inner.call(req).await
        })
    }
}
