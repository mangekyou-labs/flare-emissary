use alloy::primitives::{Log, B256};
use chrono::{DateTime, Utc};
use flare_common::types::{Chain, DecodedEvent, EventType};
use serde_json::json;

use crate::EventDecoder;

/// Generic EVM event decoder.
///
/// This decoder captures any event that isn't matched by the protocol-specific
/// decoders. It stores the raw topics and data for user-defined ABI decoding later.
pub struct GenericDecoder;

impl GenericDecoder {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GenericDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl EventDecoder for GenericDecoder {
    fn event_signatures(&self) -> Vec<B256> {
        // Generic decoder doesn't filter by signature — it catches everything.
        // The registry calls it last as a fallback.
        vec![]
    }

    fn decode(
        &self,
        log: &Log,
        block_number: u64,
        block_timestamp: DateTime<Utc>,
        chain: Chain,
    ) -> Option<DecodedEvent> {
        let topic0 = log.topics().first()?;
        let address = format!("{:#x}", log.address);

        let topics: Vec<String> = log.topics().iter().map(|t| format!("{:#x}", t)).collect();

        Some(DecodedEvent {
            tx_hash: String::new(),
            log_index: None,
            block_number,
            block_timestamp,
            chain,
            address,
            event_type: EventType::GenericEvent,
            decoded_data: json!({
                "topic0": format!("{:#x}", topic0),
                "topics": topics,
                "data": format!("0x{}", alloy::hex::encode(log.data.data.as_ref())),
            }),
        })
    }

    fn name(&self) -> &'static str {
        "Generic"
    }
}
