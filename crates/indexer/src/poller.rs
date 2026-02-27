use std::time::Duration;

use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use chrono::{TimeZone, Utc};
use sqlx::PgPool;

use flare_common::types::{Chain, DecodedEvent};
use flare_decoders::DecoderRegistry;

use crate::reorg::ReorgDetector;

/// Block poller that continuously fetches new blocks and logs from the chain.
pub struct BlockPoller {
    rpc_url: String,
    poll_interval: Duration,
    chain: Chain,
    pool: PgPool,
    decoders: DecoderRegistry,
    reorg_detector: ReorgDetector,
    /// Optional list of contract addresses to filter logs by.
    /// When set, only logs from these addresses are fetched.
    /// When empty, ALL logs for the block are fetched (unfiltered).
    contract_addresses: Vec<Address>,
}

impl BlockPoller {
    pub fn new(
        rpc_url: String,
        poll_interval_ms: u64,
        chain: Chain,
        pool: PgPool,
        reorg_window: u64,
    ) -> Self {
        Self {
            rpc_url,
            poll_interval: Duration::from_millis(poll_interval_ms),
            chain,
            pool,
            decoders: DecoderRegistry::new(),
            reorg_detector: ReorgDetector::new(reorg_window as usize),
            contract_addresses: Vec::new(),
        }
    }

    /// Set the contract addresses to filter logs by.
    /// Only logs emitted by these contracts will be fetched and decoded.
    pub fn with_contract_addresses(mut self, addresses: Vec<Address>) -> Self {
        if !addresses.is_empty() {
            tracing::info!(
                count = addresses.len(),
                "Log filtering enabled — only fetching logs from resolved contract addresses"
            );
        }
        self.contract_addresses = addresses;
        self
    }

    /// Start the polling loop. Runs indefinitely until the task is cancelled.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        let provider = ProviderBuilder::new().connect_http(self.rpc_url.parse()?);

        // Determine starting block
        let mut current_block = self.get_last_indexed_block().await?.unwrap_or_else(|| {
            tracing::info!("No previous indexed block found, starting from latest");
            0 // Will be set to latest below
        });

        if current_block == 0 {
            let latest = provider.get_block_number().await?;
            current_block = latest;
            tracing::info!(block = current_block, "Starting from latest block");
        }

        tracing::info!(
            chain = %self.chain,
            start_block = current_block,
            poll_interval_ms = self.poll_interval.as_millis() as u64,
            "Block poller started"
        );

        loop {
            match self.poll_block(&provider, current_block).await {
                Ok(events) => {
                    if !events.is_empty() {
                        tracing::info!(
                            block = current_block,
                            events = events.len(),
                            "Decoded events from block"
                        );
                        self.persist_events(&events).await?;
                    }
                    // Always update indexer state, even for blocks with no events
                    self.update_indexer_state(current_block).await?;
                    current_block += 1;
                }
                Err(e) => {
                    // Block might not exist yet — wait and retry
                    tracing::debug!(
                        block = current_block,
                        error = %e,
                        "Block not yet available, waiting..."
                    );
                    tokio::time::sleep(self.poll_interval).await;
                    continue;
                }
            }

            // Brief sleep to avoid hammering the RPC when caught up
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Poll a single block: fetch it, check for reorg, fetch logs, decode events.
    async fn poll_block(
        &mut self,
        provider: &impl Provider,
        block_number: u64,
    ) -> anyhow::Result<Vec<DecodedEvent>> {
        // Fetch block header for hash and timestamp
        let block = provider
            .get_block_by_number(block_number.into())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Block {} not found", block_number))?;

        let block_hash = block.header.hash;
        let parent_hash = block.header.parent_hash;
        let block_timestamp = Utc
            .timestamp_opt(block.header.timestamp as i64, 0)
            .single()
            .unwrap_or_else(Utc::now);

        // Check for reorg (pass parent_hash to avoid a second RPC call)
        if let Some(reorg_block) = self
            .reorg_detector
            .check_and_record(block_number, block_hash, parent_hash, provider)
            .await?
        {
            tracing::warn!(
                reorg_at = reorg_block,
                current = block_number,
                "Reorg detected! Rolling back events."
            );
            self.rollback_events_from(reorg_block).await?;
            return Ok(vec![]);
        }

        // Fetch logs for this block (filtered by contract addresses if available)
        let mut filter = Filter::new()
            .from_block(block_number)
            .to_block(block_number);

        if !self.contract_addresses.is_empty() {
            filter = filter.address(self.contract_addresses.clone());
        }

        let logs = provider.get_logs(&filter).await?;

        if logs.is_empty() {
            return Ok(vec![]);
        }

        // Decode each log
        let mut events = Vec::new();
        for log in &logs {
            let tx_hash = log
                .transaction_hash
                .map(|h| format!("{:#x}", h))
                .unwrap_or_default();

            let log_index = log.log_index;

            if let Some(mut event) =
                self.decoders
                    .decode(&log.inner, block_number, block_timestamp, self.chain)
            {
                event.tx_hash = tx_hash;
                event.log_index = log_index;
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Persist decoded events to the database.
    pub async fn persist_events(&self, events: &[DecodedEvent]) -> anyhow::Result<()> {
        for event in events {
            sqlx::query(
                r#"
                INSERT INTO indexed_events (tx_hash, log_index, block_number, block_timestamp, chain, address, event_type, decoded_data, is_reorged)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, false)
                ON CONFLICT (tx_hash, log_index) DO NOTHING
                "#,
            )
            .bind(&event.tx_hash)
            .bind(event.log_index.map(|i| i as i64))
            .bind(event.block_number as i64)
            .bind(event.block_timestamp)
            .bind(event.chain.to_string())
            .bind(&event.address)
            .bind(event.event_type.to_string())
            .bind(&event.decoded_data)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Update the indexer's last processed block number.
    pub async fn update_indexer_state(&self, block_number: u64) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO indexer_state (chain, last_block)
            VALUES ($1, $2)
            ON CONFLICT (chain) DO UPDATE SET last_block = $2, updated_at = NOW()
            "#,
        )
        .bind(self.chain.to_string())
        .bind(block_number as i64)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get the last indexed block number from the database.
    pub async fn get_last_indexed_block(&self) -> anyhow::Result<Option<u64>> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT last_block FROM indexer_state WHERE chain = $1")
                .bind(self.chain.to_string())
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(|(b,)| b as u64))
    }

    /// Mark events from a reorged block as invalid.
    pub async fn rollback_events_from(&self, from_block: u64) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE indexed_events SET is_reorged = true WHERE block_number >= $1 AND chain = $2",
        )
        .bind(from_block as i64)
        .bind(self.chain.to_string())
        .execute(&self.pool)
        .await?;

        tracing::info!(from_block, "Rolled back events from reorged blocks");
        Ok(())
    }
}
