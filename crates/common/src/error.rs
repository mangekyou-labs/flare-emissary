use thiserror::Error;

/// Common error types used across the application.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("Decode error: {0}")]
    Decode(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
