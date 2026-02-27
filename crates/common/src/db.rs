use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Create a PostgreSQL connection pool.
///
/// `max_connections` controls the maximum number of connections in the pool.
/// Pass `AppConfig::db_max_connections` for the user-configured value (default 20).
pub async fn create_pool(database_url: &str, max_connections: u32) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(database_url)
        .await?;

    tracing::info!(max_connections, "Connected to PostgreSQL");
    Ok(pool)
}
