use serde::Deserialize;

/// Global application configuration loaded from environment variables.
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    /// Primary Flare RPC URL (NaaS provider recommended)
    pub flare_rpc_url: String,

    /// Fallback Flare RPC URL (public endpoint)
    pub flare_rpc_fallback_url: Option<String>,

    /// PostgreSQL connection string
    pub database_url: String,

    /// Redis connection string
    pub redis_url: String,

    /// Block polling interval in milliseconds (default: 1500 for < 1.8s blocks)
    pub indexer_poll_interval_ms: u64,

    /// Number of recent block hashes to track for reorg detection
    pub indexer_reorg_window: u64,

    /// JWT secret for API authentication
    pub jwt_secret: String,

    /// JWT token expiry in hours
    pub jwt_expiry_hours: u64,

    /// Telegram bot token
    pub telegram_bot_token: Option<String>,

    /// Discord bot token
    pub discord_bot_token: Option<String>,

    /// Resend API key for email delivery
    pub resend_api_key: Option<String>,

    /// Email sender address
    pub email_from: Option<String>,

    /// Maximum number of PostgreSQL connections in the pool (default: 20)
    pub db_max_connections: u32,
}

impl AppConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Self {
            flare_rpc_url: std::env::var("FLARE_RPC_URL")
                .unwrap_or_else(|_| "https://flare-api.flare.network/ext/C/rpc".to_string()),
            flare_rpc_fallback_url: std::env::var("FLARE_RPC_FALLBACK_URL").ok(),
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL environment variable is required"))?,
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            indexer_poll_interval_ms: std::env::var("INDEXER_POLL_INTERVAL_MS")
                .unwrap_or_else(|_| "1500".to_string())
                .parse()
                .map_err(|_| anyhow::anyhow!("INDEXER_POLL_INTERVAL_MS must be a valid u64"))?,
            indexer_reorg_window: std::env::var("INDEXER_REORG_WINDOW")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .map_err(|_| anyhow::anyhow!("INDEXER_REORG_WINDOW must be a valid u64"))?,
            jwt_secret: std::env::var("JWT_SECRET")
                .map_err(|_| anyhow::anyhow!("JWT_SECRET environment variable is required"))?,
            jwt_expiry_hours: std::env::var("JWT_EXPIRY_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()
                .map_err(|_| anyhow::anyhow!("JWT_EXPIRY_HOURS must be a valid u64"))?,
            telegram_bot_token: std::env::var("TELEGRAM_BOT_TOKEN").ok(),
            discord_bot_token: std::env::var("DISCORD_BOT_TOKEN").ok(),
            resend_api_key: std::env::var("RESEND_API_KEY").ok(),
            email_from: std::env::var("EMAIL_FROM").ok(),
            db_max_connections: std::env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .map_err(|_| anyhow::anyhow!("DB_MAX_CONNECTIONS must be a valid u32"))?,
        })
    }
}
