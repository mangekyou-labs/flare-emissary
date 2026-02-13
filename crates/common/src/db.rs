use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Create a PostgreSQL connection pool.
pub async fn create_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(database_url)
        .await?;

    tracing::info!("Connected to PostgreSQL");
    Ok(pool)
}
