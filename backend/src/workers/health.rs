use std::sync::Arc;
use tracing::{info, debug, warn, error};
use redis::{Client, AsyncCommands};
use sqlx::PgPool;
use serde::{Serialize, Deserialize};
use std::time::{Duration, Instant};

/// Worker health monitor that tracks and reports worker status
#[derive(Debug, Clone)]
pub struct WorkerHealthMonitor {
    db_pool: PgPool,
    redis_client: Client,
    check_interval: Duration,
    health_ttl: Duration,
}

impl WorkerHealthMonitor {
    /// Create a new worker health monitor
    pub fn new(db_pool: PgPool, redis_client: Client, check_interval: Duration, health_ttl: Duration) -> Self {
        Self {
            db_pool,
            redis_client,
            check_interval,
            health_ttl,
        }
    }

    /// Start the health monitoring process
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting worker health monitor...");
        
        loop {
            if let Err(e) = self.check_health().await {
                error!("Failed to check worker health: {}", e);
            }
            
            // Wait for the configured interval before next health check
            tokio::time::sleep(self.check_interval).await;
        }
    }

    /// Check health of all workers
    async fn check_health(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Checking worker health...");
        
        // Get Redis connection
        let mut redis_conn = self.redis_client.get_async_connection().await?;
        
        // Get list of worker health keys
        let health_keys: Vec<String> = redis_conn.keys("worker:*:health").await?;
        
        for key in health_keys {
            // Get current health data
            let health_data: Option<String> = redis_conn.get(&key).await?;
            
            if let Some(json) = health_data {
                match serde_json::from_str::<WorkerHealth>(&json) {
                    Ok(mut health) => {
                        // Update last checked timestamp
                        health.last_checked = Instant::now();
                        
                        // Calculate health status based on last heartbeat
                        let elapsed = health.last_heartbeat.elapsed();
                        health.is_healthy = elapsed < Duration::from_secs(30); // 30 seconds threshold
                        
                        // Store updated health status
                        redis_conn.set_ex(
                            &key,
                            serde_json::to_string(&health)?,
                            self.health_ttl.as_secs() as usize,
                        ).await?;
                        
                        debug!("Updated health for worker: {} (healthy: {})", health.worker_id, health.is_healthy);
                    }
                    Err(e) => {
                        error!("Failed to parse health data for {}: {}", key, e);
                    }
                }
            }
        }
        
        info!("Worker health check completed");
        Ok(())
    }

    /// Report worker heartbeat
    pub async fn report_heartbeat(&self, worker_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut redis_conn = self.redis_client.get_async_connection().await?;
        let key = format!("worker:{}:health", worker_id);
        
        let health = WorkerHealth {
            worker_id: worker_id.to_string(),
            last_heartbeat: Instant::now(),
            last_checked: Instant::now(),
            is_healthy: true,
            uptime_seconds: 0,
        };
        
        redis_conn.set_ex(
            &key,
            serde_json::to_string(&health)?,
            self.health_ttl.as_secs() as usize,
        ).await?;
        
        Ok(())
    }

    /// Get health status for a specific worker
    pub async fn get_worker_health(&self, worker_id: &str) -> Result<Option<WorkerHealth>, Box<dyn std::error::Error>> {
        let mut redis_conn = self.redis_client.get_async_connection().await?;
        let key = format!("worker:{}:health", worker_id);
        
        let health_data: Option<String> = redis_conn.get(&key).await?;
        
        match health_data {
            Some(json) => {
                let health: WorkerHealth = serde_json::from_str(&json)?;
                Ok(Some(health))
            }
            None => Ok(None),
        }
    }
}

/// Worker health structure
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkerHealth {
    pub worker_id: String,
    pub last_heartbeat: std::time::Instant,
    pub last_checked: std::time::Instant,
    pub is_healthy: bool,
    pub uptime_seconds: u64,
}

// Implement custom serialization for Instant
impl Serialize for std::time::Instant {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.elapsed().as_secs().to_string())
    }
}

impl<'de> Deserialize<'de> for std::time::Instant {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let secs: u64 = serde::Deserialize::deserialize(deserializer)?;
        Ok(std::time::Instant::now() - std::time::Duration::from_secs(secs))
    }
}
