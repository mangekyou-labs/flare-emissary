use alloy::primitives::{B256, Log, keccak256};
use chrono::{DateTime, Utc};
use flare_common::types::{Chain, DecodedEvent, EventType};
use serde_json::json;

use crate::EventDecoder;

/// Flare Data Connector (FDC) event decoder.
///
/// Handles key FDC attestation lifecycle events:
/// - `AttestationRequested(bytes32 requestId, address requester)`
/// - `AttestationProved(bytes32 requestId, bytes32 merkleRoot)`
/// - `RoundFinalized(uint256 roundId, bytes32 merkleRoot)`
pub struct FdcDecoder {
    attestation_requested: B256,
    attestation_proved: B256,
    round_finalized: B256,
}

impl FdcDecoder {
    pub fn new() -> Self {
        Self {
            attestation_requested: keccak256("AttestationRequested(bytes32,address)"),
            attestation_proved: keccak256("AttestationProved(bytes32,bytes32)"),
            round_finalized: keccak256("RoundFinalized(uint256,bytes32)"),
        }
    }
}

impl Default for FdcDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl EventDecoder for FdcDecoder {
    fn event_signatures(&self) -> Vec<B256> {
        vec![
            self.attestation_requested,
            self.attestation_proved,
            self.round_finalized,
        ]
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

        if *topic0 == self.attestation_requested {
            let request_id = log.topics().get(1).map(|t| format!("{:#x}", t));
            let requester = log
                .topics()
                .get(2)
                .map(|t| format!("0x{}", alloy::hex::encode(&t.as_slice()[12..32])));

            Some(DecodedEvent {
                tx_hash: String::new(),
                log_index: None,
                block_number,
                block_timestamp,
                chain,
                address,
                event_type: EventType::AttestationRequested,
                decoded_data: json!({
                    "request_id": request_id,
                    "requester": requester,
                }),
            })
        } else if *topic0 == self.attestation_proved {
            let request_id = log.topics().get(1).map(|t| format!("{:#x}", t));
            let merkle_root = log.topics().get(2).map(|t| format!("{:#x}", t));

            Some(DecodedEvent {
                tx_hash: String::new(),
                log_index: None,
                block_number,
                block_timestamp,
                chain,
                address,
                event_type: EventType::AttestationProved,
                decoded_data: json!({
                    "request_id": request_id,
                    "merkle_root": merkle_root,
                }),
            })
        } else if *topic0 == self.round_finalized {
            let round_id = log.topics().get(1).map(|t| {
                let bytes = t.as_slice();
                u64::from_be_bytes(bytes[24..32].try_into().unwrap_or_default())
            });

            Some(DecodedEvent {
                tx_hash: String::new(),
                log_index: None,
                block_number,
                block_timestamp,
                chain,
                address,
                event_type: EventType::RoundFinalized,
                decoded_data: json!({
                    "round_id": round_id,
                }),
            })
        } else {
            None
        }
    }

    fn name(&self) -> &'static str {
        "FDC"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_signatures() {
        let decoder = FdcDecoder::new();
        assert_eq!(decoder.event_signatures().len(), 3);
    }
}
