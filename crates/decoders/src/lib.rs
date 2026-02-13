pub mod fasset;
pub mod fdc;
pub mod ftso;
pub mod generic;

use alloy::primitives::Log;
use chrono::{DateTime, Utc};
use flare_common::types::{Chain, DecodedEvent};

/// Trait that all protocol-specific decoders must implement.
pub trait EventDecoder: Send + Sync {
    /// Returns the event topic signatures this decoder handles.
    fn event_signatures(&self) -> Vec<alloy::primitives::B256>;

    /// Attempt to decode a raw log entry into a `DecodedEvent`.
    /// Returns `None` if this decoder doesn't handle the log's topic.
    fn decode(
        &self,
        log: &Log,
        block_number: u64,
        block_timestamp: DateTime<Utc>,
        chain: Chain,
    ) -> Option<DecodedEvent>;

    /// Human-readable name for this decoder (e.g., "FTSO v2").
    fn name(&self) -> &'static str;
}

/// Registry of all available decoders, used by the indexer's event router.
pub struct DecoderRegistry {
    decoders: Vec<Box<dyn EventDecoder>>,
}

impl DecoderRegistry {
    /// Create a new registry with all protocol decoders.
    ///
    /// Note: `GenericDecoder` is NOT included by default to avoid flooding the
    /// database with every unmatched event on the chain. Use `with_generic()`
    /// to add it for specific monitored addresses.
    pub fn new() -> Self {
        Self {
            decoders: vec![
                Box::new(ftso::FtsoDecoder::new()),
                Box::new(fdc::FdcDecoder::new()),
                Box::new(fasset::FassetDecoder::new()),
            ],
        }
    }

    /// Create a registry that also captures generic (unmatched) events.
    /// Use with caution — this will persist every log on the chain.
    pub fn with_generic() -> Self {
        Self {
            decoders: vec![
                Box::new(ftso::FtsoDecoder::new()),
                Box::new(fdc::FdcDecoder::new()),
                Box::new(fasset::FassetDecoder::new()),
                Box::new(generic::GenericDecoder::new()),
            ],
        }
    }

    /// Try to decode a log using all registered decoders.
    /// Returns the first successful decode, or `None`.
    pub fn decode(
        &self,
        log: &Log,
        block_number: u64,
        block_timestamp: DateTime<Utc>,
        chain: Chain,
    ) -> Option<DecodedEvent> {
        for decoder in &self.decoders {
            if let Some(event) = decoder.decode(log, block_number, block_timestamp, chain) {
                tracing::debug!(
                    decoder = decoder.name(),
                    event_type = %event.event_type,
                    "Decoded event"
                );
                return Some(event);
            }
        }
        None
    }

    /// Get all event signatures across all registered decoders.
    pub fn all_signatures(&self) -> Vec<alloy::primitives::B256> {
        self.decoders
            .iter()
            .flat_map(|d| d.event_signatures())
            .collect()
    }
}

impl Default for DecoderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
