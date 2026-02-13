use redis::aio::ConnectionManager;
use redis::Client;

/// Create a Redis connection manager for async operations.
pub async fn create_redis_pool(redis_url: &str) -> anyhow::Result<ConnectionManager> {
    let client = Client::open(redis_url)?;
    let manager = ConnectionManager::new(client).await?;

    tracing::info!("Connected to Redis");
    Ok(manager)
}
