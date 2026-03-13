use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct DbManager {
    primary: PgPool,
    secondary: Option<PgPool>,
    write_queue: mpsc::UnboundedSender<QueuedWrite>,
}

struct QueuedWrite {
    query: String,
    params: serde_json::Value, // Simplified for demonstration
    retries: u32,
}

impl DbManager {
    pub fn new(primary: PgPool, secondary: Option<PgPool>) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<QueuedWrite>();
        let primary_clone = primary.clone();
        let tx_retry = tx.clone();
        
        tokio::spawn(async move {
            while let Some(mut item) = rx.recv().await {
                // In a production app, we would use a more robust WAL or Outbox table
                match sqlx::query(&item.query)
                    .execute(&primary_clone)
                    .await 
                {
                    Ok(_) => info!("Resilient write synced to primary"),
                    Err(e) => {
                        item.retries += 1;
                        if item.retries < 10 {
                            warn!("Primary write failed, retrying in background ({}): {}", item.retries, e);
                            let _ = tx_retry.send(item);
                            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        } else {
                            error!("CRITICAL: Background write failed after 10 retries: {}", e);
                        }
                    }
                }
            }
        });

        Self { primary, secondary, write_queue: tx }
    }

    /// Primary pool for direct access if needed
    pub fn primary(&self) -> &PgPool {
        &self.primary
    }

    /// Get a pool suitable for reading, with failover to secondary
    pub async fn read_pool(&self) -> &PgPool {
        match sqlx::query("SELECT 1").execute(&self.primary).await {
            Ok(_) => &self.primary,
            Err(_) => {
                if let Some(sec) = &self.secondary {
                    warn!("Primary unavailable, switching to secondary for READ");
                    sec
                } else {
                    &self.primary
                }
            }
        }
    }

    /// Get a pool for writing (always primary)
    pub fn write_pool(&self) -> &PgPool {
        &self.primary
    }

    /// Execute a write that will be retried if primary is down
    pub async fn resilient_write(&self, query: &str) {
        if let Err(e) = sqlx::query(query).execute(&self.primary).await {
            warn!("Direct write failed, queuing for retry: {}", e);
            let _ = self.write_queue.send(QueuedWrite {
                query: query.to_string(),
                params: serde_json::Value::Null,
                retries: 0,
            });
        }
    }
}
