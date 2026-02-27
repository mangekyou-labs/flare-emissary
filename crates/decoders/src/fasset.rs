use alloy::primitives::{B256, Log, keccak256};
use chrono::{DateTime, Utc};
use flare_common::types::{Chain, DecodedEvent, EventType};
use serde_json::json;

use crate::EventDecoder;

/// FAsset event decoder.
///
/// Handles key FAsset Agent/Vault lifecycle events:
/// - `CollateralDeposited(address agent, uint256 amount)`
/// - `CollateralWithdrawn(address agent, uint256 amount)`
/// - `MintingExecuted(address minter, address agent, uint256 lots)`
/// - `RedemptionRequested(address redeemer, address agent, uint256 lots)`
/// - `LiquidationStarted(address agent, uint256 timestamp)`
pub struct FassetDecoder {
    collateral_deposited: B256,
    collateral_withdrawn: B256,
    minting_executed: B256,
    redemption_requested: B256,
    liquidation_started: B256,
}

impl FassetDecoder {
    pub fn new() -> Self {
        Self {
            collateral_deposited: keccak256("CollateralDeposited(address,uint256)"),
            collateral_withdrawn: keccak256("CollateralWithdrawn(address,uint256)"),
            minting_executed: keccak256("MintingExecuted(address,address,uint256)"),
            redemption_requested: keccak256("RedemptionRequested(address,address,uint256)"),
            liquidation_started: keccak256("LiquidationStarted(address,uint256)"),
        }
    }

    fn decode_address_from_topic(topic: &B256) -> String {
        format!("0x{}", alloy::hex::encode(&topic.as_slice()[12..32]))
    }

    fn decode_u256_from_data(data: &[u8], offset: usize) -> Option<String> {
        if data.len() >= offset + 32 {
            let bytes: [u8; 32] = data[offset..offset + 32].try_into().ok()?;
            Some(alloy::primitives::U256::from_be_bytes(bytes).to_string())
        } else {
            None
        }
    }
}

impl Default for FassetDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl EventDecoder for FassetDecoder {
    fn event_signatures(&self) -> Vec<B256> {
        vec![
            self.collateral_deposited,
            self.collateral_withdrawn,
            self.minting_executed,
            self.redemption_requested,
            self.liquidation_started,
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
        let data = log.data.data.as_ref();

        if *topic0 == self.collateral_deposited {
            let agent = log.topics().get(1).map(Self::decode_address_from_topic);
            let amount = Self::decode_u256_from_data(data, 0);

            Some(DecodedEvent {
                tx_hash: String::new(),
                log_index: None,
                block_number,
                block_timestamp,
                chain,
                address,
                event_type: EventType::CollateralDeposited,
                decoded_data: json!({ "agent": agent, "amount": amount }),
            })
        } else if *topic0 == self.collateral_withdrawn {
            let agent = log.topics().get(1).map(Self::decode_address_from_topic);
            let amount = Self::decode_u256_from_data(data, 0);

            Some(DecodedEvent {
                tx_hash: String::new(),
                log_index: None,
                block_number,
                block_timestamp,
                chain,
                address,
                event_type: EventType::CollateralWithdrawn,
                decoded_data: json!({ "agent": agent, "amount": amount }),
            })
        } else if *topic0 == self.minting_executed {
            let minter = log.topics().get(1).map(Self::decode_address_from_topic);
            let agent = log.topics().get(2).map(Self::decode_address_from_topic);
            let lots = Self::decode_u256_from_data(data, 0);

            Some(DecodedEvent {
                tx_hash: String::new(),
                log_index: None,
                block_number,
                block_timestamp,
                chain,
                address,
                event_type: EventType::MintingExecuted,
                decoded_data: json!({ "minter": minter, "agent": agent, "lots": lots }),
            })
        } else if *topic0 == self.redemption_requested {
            let redeemer = log.topics().get(1).map(Self::decode_address_from_topic);
            let agent = log.topics().get(2).map(Self::decode_address_from_topic);
            let lots = Self::decode_u256_from_data(data, 0);

            Some(DecodedEvent {
                tx_hash: String::new(),
                log_index: None,
                block_number,
                block_timestamp,
                chain,
                address,
                event_type: EventType::RedemptionRequested,
                decoded_data: json!({ "redeemer": redeemer, "agent": agent, "lots": lots }),
            })
        } else if *topic0 == self.liquidation_started {
            let agent = log.topics().get(1).map(Self::decode_address_from_topic);

            Some(DecodedEvent {
                tx_hash: String::new(),
                log_index: None,
                block_number,
                block_timestamp,
                chain,
                address,
                event_type: EventType::LiquidationStarted,
                decoded_data: json!({ "agent": agent }),
            })
        } else {
            None
        }
    }

    fn name(&self) -> &'static str {
        "FAsset"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_signatures() {
        let decoder = FassetDecoder::new();
        assert_eq!(decoder.event_signatures().len(), 5);
    }
}
