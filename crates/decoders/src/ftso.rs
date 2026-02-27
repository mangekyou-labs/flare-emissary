use alloy::primitives::{B256, Log, keccak256};
use chrono::{DateTime, Utc};
use flare_common::types::{Chain, DecodedEvent, EventType};
use serde_json::json;

use crate::EventDecoder;

/// FTSO v2 event decoder.
///
/// Handles key FTSO events:
/// - `PriceEpochFinalized(uint256 epochId, uint256 timestamp)`
/// - `VotePowerChanged(address provider, uint256 newVotePower)`
/// - `RewardEpochStarted(uint256 epochId, uint256 timestamp)`
pub struct FtsoDecoder {
    price_epoch_finalized: B256,
    vote_power_changed: B256,
    reward_epoch_started: B256,
}

impl FtsoDecoder {
    pub fn new() -> Self {
        Self {
            price_epoch_finalized: keccak256("PriceEpochFinalized(uint256,uint256)"),
            vote_power_changed: keccak256("VotePowerChanged(address,uint256)"),
            reward_epoch_started: keccak256("RewardEpochStarted(uint256,uint256)"),
        }
    }
}

impl Default for FtsoDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl EventDecoder for FtsoDecoder {
    fn event_signatures(&self) -> Vec<B256> {
        vec![
            self.price_epoch_finalized,
            self.vote_power_changed,
            self.reward_epoch_started,
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

        if *topic0 == self.price_epoch_finalized {
            // Decode epoch ID from topic[1] if available, otherwise from data
            let epoch_id = log.topics().get(1).map(|t| {
                let bytes = t.as_slice();
                u64::from_be_bytes(bytes[24..32].try_into().unwrap_or_default())
            });

            Some(DecodedEvent {
                tx_hash: String::new(),
                log_index: None, // Set by caller
                block_number,
                block_timestamp,
                chain,
                address,
                event_type: EventType::PriceEpochFinalized,
                decoded_data: json!({
                    "epoch_id": epoch_id,
                    "raw_data": format!("0x{}", alloy::hex::encode(log.data.data.as_ref())),
                }),
            })
        } else if *topic0 == self.vote_power_changed {
            let provider = log
                .topics()
                .get(1)
                .map(|t| format!("0x{}", alloy::hex::encode(&t.as_slice()[12..32])));
            let new_vote_power = if log.data.data.len() >= 32 {
                let bytes: [u8; 32] = log.data.data.as_ref()[..32].try_into().unwrap_or_default();
                Some(alloy::primitives::U256::from_be_bytes(bytes).to_string())
            } else {
                None
            };

            Some(DecodedEvent {
                tx_hash: String::new(),
                log_index: None,
                block_number,
                block_timestamp,
                chain,
                address,
                event_type: EventType::VotePowerChanged,
                decoded_data: json!({
                    "provider": provider,
                    "new_vote_power": new_vote_power,
                }),
            })
        } else if *topic0 == self.reward_epoch_started {
            let epoch_id = log.topics().get(1).map(|t| {
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
                event_type: EventType::RewardEpochStarted,
                decoded_data: json!({
                    "epoch_id": epoch_id,
                }),
            })
        } else {
            None
        }
    }

    fn name(&self) -> &'static str {
        "FTSO v2"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_signatures_not_empty() {
        let decoder = FtsoDecoder::new();
        assert_eq!(decoder.event_signatures().len(), 3);
    }

    #[test]
    fn test_name() {
        let decoder = FtsoDecoder::new();
        assert_eq!(decoder.name(), "FTSO v2");
    }
}
