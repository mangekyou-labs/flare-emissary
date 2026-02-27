use alloy::providers::ProviderBuilder;

use flare_common::config::AppConfig;
use flare_common::db;
use flare_common::types::Chain;
use flare_indexer::poller::BlockPoller;
use flare_indexer::registry::FlareContractRegistry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "flare_indexer=info,flare_decoders=debug".into()),
        )
        .json()
        .init();

    tracing::info!("FlareEmissary Indexer starting...");

    // Load configuration
    let config = AppConfig::from_env()?;

    // Connect to database
    let pool = db::create_pool(&config.database_url, config.db_max_connections).await?;

    // Run migrations
    sqlx::migrate!("../../migrations").run(&pool).await?;
    tracing::info!("Database migrations applied");

    // Resolve contract addresses from the on-chain FlareContractRegistry
    tracing::info!("Resolving contract addresses from FlareContractRegistry...");
    let provider = ProviderBuilder::new().connect_http(config.flare_rpc_url.parse()?);
    let registry = FlareContractRegistry::flare();
    let resolved = match registry.resolve_all(&provider).await {
        Ok(resolved) => {
            tracing::info!(
                count = resolved.len(),
                "Contract address resolution successful"
            );
            resolved.all_addresses()
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "Failed to resolve contract addresses — falling back to unfiltered log fetching"
            );
            Vec::new()
        }
    };

    // Start block poller for Flare mainnet
    let mut poller = BlockPoller::new(
        config.flare_rpc_url.clone(),
        config.indexer_poll_interval_ms,
        Chain::Flare,
        pool,
        config.indexer_reorg_window,
    )
    .with_contract_addresses(resolved);

    tracing::info!("Starting block poller for Flare mainnet");

    // Run with graceful shutdown on Ctrl+C
    tokio::select! {
        result = poller.run() => {
            if let Err(e) = result {
                tracing::error!(error = %e, "Block poller exited with error");
                return Err(e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received shutdown signal, stopping gracefully...");
        }
    }

    tracing::info!("FlareEmissary Indexer stopped.");
    Ok(())
}
