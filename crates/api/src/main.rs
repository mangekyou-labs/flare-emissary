//! FlareEmissary API server binary entrypoint.

use std::net::SocketAddr;

use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use flare_common::config::AppConfig;
use flare_common::db::create_pool;
use flare_common::redis_pool::create_redis_pool;

use flare_api::routes::create_router;
use flare_api::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("flare_api=debug,flare_engine=debug,tower_http=debug")
        }))
        .init();

    tracing::info!("Starting FlareEmissary API server...");

    // Load configuration
    let config = AppConfig::from_env()?;

    // Create database connection pool
    let pool = create_pool(&config.database_url, config.db_max_connections).await?;
    tracing::info!("Database pool created");

    // Create Redis connection
    let redis = create_redis_pool(&config.redis_url).await?;
    tracing::info!("Redis connection established");

    // Build application state
    let state = AppState::new(pool, redis, config);

    // Build router
    let app = create_router(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("API server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
