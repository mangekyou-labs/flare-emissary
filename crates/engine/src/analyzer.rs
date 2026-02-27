//! Address analyzer — classifies blockchain addresses and discovers subscribable events.
//!
//! Classification approach:
//! 1. Check local DB cache for known addresses
//! 2. Determine address type: FTSO provider, FAsset agent, generic contract, or EOA
//! 3. Return the list of event types this address can emit

use sqlx::PgPool;

use flare_common::error::AppError;
use flare_common::types::{AddressType, EventType, MonitoredAddress};

/// Result of analyzing an address.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AddressClassification {
    /// The address that was analyzed
    pub address: String,
    /// Detected address type
    pub address_type: AddressType,
    /// Events this address can emit that users can subscribe to
    pub subscribable_events: Vec<EventType>,
    /// Human-readable label (e.g., "FTSO Data Provider", "FAsset Agent")
    pub label: String,
}

/// Address analyzer service.
pub struct AddressAnalyzer;

impl AddressAnalyzer {
    /// Classify an address by checking the local DB cache.
    ///
    /// If the address is already in `monitored_addresses`, use the cached type.
    /// Otherwise, default to `GenericContract` and insert it for future tracking.
    pub async fn classify(
        address: &str,
        chain: &str,
        pool: &PgPool,
    ) -> Result<AddressClassification, AppError> {
        // Check DB cache first
        let existing: Option<MonitoredAddress> =
            sqlx::query_as("SELECT * FROM monitored_addresses WHERE address = $1 AND chain = $2")
                .bind(address)
                .bind(chain)
                .fetch_optional(pool)
                .await?;

        if let Some(monitored) = existing {
            let subscribable = Self::events_for_type(&monitored.address_type);
            let label = Self::label_for_type(&monitored.address_type);
            return Ok(AddressClassification {
                address: monitored.address,
                address_type: monitored.address_type,
                subscribable_events: subscribable,
                label,
            });
        }

        // Address not yet monitored — default to GenericContract
        // In production, this would query on-chain registries to detect the type
        let address_type = AddressType::GenericContract;
        let subscribable = Self::events_for_type(&address_type);
        let label = Self::label_for_type(&address_type);

        // Insert into monitored_addresses for future lookups
        let detected_events: Vec<String> = subscribable.iter().map(|e| e.to_string()).collect();
        sqlx::query(
            r#"
            INSERT INTO monitored_addresses (address, chain, address_type, detected_events)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (address, chain) DO NOTHING
            "#,
        )
        .bind(address)
        .bind(chain)
        .bind(address_type.to_string())
        .bind(serde_json::json!(detected_events))
        .execute(pool)
        .await?;

        Ok(AddressClassification {
            address: address.to_string(),
            address_type,
            subscribable_events: subscribable,
            label,
        })
    }

    /// Get the subscribable event types for a given address type.
    pub fn events_for_type(address_type: &AddressType) -> Vec<EventType> {
        match address_type {
            AddressType::FtsoProvider => vec![
                EventType::PriceEpochFinalized,
                EventType::VotePowerChanged,
                EventType::RewardEpochStarted,
            ],
            AddressType::FassetAgent => vec![
                EventType::CollateralDeposited,
                EventType::CollateralWithdrawn,
                EventType::MintingExecuted,
                EventType::RedemptionRequested,
                EventType::LiquidationStarted,
            ],
            AddressType::GenericContract => vec![EventType::GenericEvent],
            AddressType::Eoa => vec![],
        }
    }

    /// Get a human-readable label for an address type.
    fn label_for_type(address_type: &AddressType) -> String {
        match address_type {
            AddressType::FtsoProvider => "FTSO Data Provider".to_string(),
            AddressType::FassetAgent => "FAsset Agent Vault".to_string(),
            AddressType::GenericContract => "Smart Contract".to_string(),
            AddressType::Eoa => "Externally Owned Account".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ftso_provider_events() {
        let events = AddressAnalyzer::events_for_type(&AddressType::FtsoProvider);
        assert_eq!(events.len(), 3);
        assert!(events.contains(&EventType::PriceEpochFinalized));
        assert!(events.contains(&EventType::VotePowerChanged));
        assert!(events.contains(&EventType::RewardEpochStarted));
    }

    #[test]
    fn test_fasset_agent_events() {
        let events = AddressAnalyzer::events_for_type(&AddressType::FassetAgent);
        assert_eq!(events.len(), 5);
        assert!(events.contains(&EventType::CollateralDeposited));
        assert!(events.contains(&EventType::LiquidationStarted));
    }

    #[test]
    fn test_generic_contract_events() {
        let events = AddressAnalyzer::events_for_type(&AddressType::GenericContract);
        assert_eq!(events.len(), 1);
        assert!(events.contains(&EventType::GenericEvent));
    }

    #[test]
    fn test_eoa_no_events() {
        let events = AddressAnalyzer::events_for_type(&AddressType::Eoa);
        assert!(events.is_empty());
    }
}
