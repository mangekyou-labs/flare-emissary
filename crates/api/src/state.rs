//! Shared application state for the Axum API server.

use flare_common::config::AppConfig;
use redis::aio::ConnectionManager;
use sqlx::PgPool;

/// Application state shared across all route handlers via Axum `State`.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub redis: ConnectionManager,
    pub config: AppConfig,
}

impl AppState {
    pub fn new(pool: PgPool, redis: ConnectionManager, config: AppConfig) -> Self {
        Self {
            pool,
            redis,
            config,
        }
    }
}
