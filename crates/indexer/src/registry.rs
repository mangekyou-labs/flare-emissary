//! FlareContractRegistry client for dynamic contract address resolution.
//!
//! Flare's `FlareContractRegistry` contract (deployed at a well-known address on
//! each network) provides `getContractAddressByName(string)` to resolve the
//! current address of any enshrined protocol contract. This makes the indexer
//! resilient to protocol upgrades that deploy new contract addresses.
//!
//! # Usage
//!
//! ```rust,no_run
//! use flare_indexer::registry::FlareContractRegistry;
//! use alloy::providers::ProviderBuilder;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let provider = ProviderBuilder::new().connect_http("https://flare-api.flare.network/ext/C/rpc".parse()?);
//! let registry = FlareContractRegistry::flare();
//! let addresses = registry.resolve_all(&provider).await?;
//! println!("Resolved {} contract addresses", addresses.len());
//! # Ok(())
//! # }
//! ```

use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::sol;
use std::collections::HashMap;
use std::str::FromStr;

// Solidity interface for the FlareContractRegistry.
// Only the function we need is defined.
sol! {
    #[sol(rpc)]
    interface IFlareContractRegistry {
        /// Returns the address of the contract with the given name.
        /// Returns address(0) if the contract is not registered.
        function getContractAddressByName(string memory _name) external view returns (address);
    }
}

/// Well-known contract names on Flare that we need for indexing.
pub const FTSO_MANAGER: &str = "FtsoManager";
pub const FTSO_REWARD_MANAGER: &str = "FtsoRewardManager";
pub const STATE_CONNECTOR: &str = "StateConnector";
pub const ASSET_MANAGER: &str = "AssetManager";
pub const VOTER_WHITELISTER: &str = "VoterWhitelister";
pub const FTSO_REGISTRY: &str = "FtsoRegistry";
pub const WNAT: &str = "WNat";

/// Well-known address of the FlareContractRegistry on Flare mainnet.
pub const FLARE_REGISTRY_ADDRESS: &str = "0xaD67FE66660Fb8dFE9d6b1b4240d8650e30F6019";

/// Well-known address of the FlareContractRegistry on Songbird.
pub const SONGBIRD_REGISTRY_ADDRESS: &str = "0xaD67FE66660Fb8dFE9d6b1b4240d8650e30F6019";

/// Set of resolved contract addresses from the FlareContractRegistry.
#[derive(Debug, Clone)]
pub struct ResolvedAddresses {
    /// Map of contract name → resolved address.
    pub addresses: HashMap<String, Address>,
}

impl ResolvedAddresses {
    /// Get a resolved address by contract name, if it exists and is non-zero.
    pub fn get(&self, name: &str) -> Option<&Address> {
        self.addresses.get(name)
    }

    /// Get all resolved addresses as a Vec (for use in log filters).
    pub fn all_addresses(&self) -> Vec<Address> {
        self.addresses.values().copied().collect()
    }

    /// Check if any addresses were successfully resolved.
    pub fn is_empty(&self) -> bool {
        self.addresses.is_empty()
    }

    /// Number of resolved addresses.
    pub fn len(&self) -> usize {
        self.addresses.len()
    }
}

/// Client for the FlareContractRegistry on-chain contract.
pub struct FlareContractRegistry {
    /// Address of the registry contract itself.
    registry_address: Address,
    /// Contract names to resolve at startup.
    contract_names: Vec<String>,
}

impl FlareContractRegistry {
    /// Create a registry client for Flare mainnet with default contract names.
    pub fn flare() -> Self {
        Self {
            registry_address: Address::from_str(FLARE_REGISTRY_ADDRESS)
                .expect("valid registry address"),
            contract_names: default_contract_names(),
        }
    }

    /// Create a registry client for Songbird with default contract names.
    pub fn songbird() -> Self {
        Self {
            registry_address: Address::from_str(SONGBIRD_REGISTRY_ADDRESS)
                .expect("valid registry address"),
            contract_names: default_contract_names(),
        }
    }

    /// Create a registry client with a custom registry address and contract names.
    pub fn custom(registry_address: Address, contract_names: Vec<String>) -> Self {
        Self {
            registry_address,
            contract_names,
        }
    }

    /// Resolve all configured contract names via the on-chain registry.
    ///
    /// Returns only contracts that resolved to non-zero addresses.
    /// Logs warnings for contracts that could not be resolved.
    pub async fn resolve_all(
        &self,
        provider: &(impl Provider + Clone),
    ) -> anyhow::Result<ResolvedAddresses> {
        let contract = IFlareContractRegistry::new(self.registry_address, provider.clone());
        let mut addresses = HashMap::new();

        for name in &self.contract_names {
            match contract.getContractAddressByName(name.clone()).call().await {
                Ok(addr) => {
                    if addr == Address::ZERO {
                        tracing::warn!(
                            contract_name = %name,
                            "Contract not registered (returned zero address)"
                        );
                    } else {
                        tracing::info!(
                            contract_name = %name,
                            address = %addr,
                            "Resolved contract address"
                        );
                        addresses.insert(name.clone(), addr);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        contract_name = %name,
                        error = %e,
                        "Failed to resolve contract address"
                    );
                }
            }
        }

        if addresses.is_empty() {
            tracing::warn!(
                "No contract addresses resolved — indexer will fetch ALL logs (unfiltered)"
            );
        } else {
            tracing::info!(
                resolved_count = addresses.len(),
                total_requested = self.contract_names.len(),
                "Contract address resolution complete"
            );
        }

        Ok(ResolvedAddresses { addresses })
    }

    /// Get the registry contract address.
    pub fn registry_address(&self) -> Address {
        self.registry_address
    }
}

/// Default contract names to resolve for the FlareEmissary indexer.
fn default_contract_names() -> Vec<String> {
    vec![
        FTSO_MANAGER.to_string(),
        FTSO_REWARD_MANAGER.to_string(),
        STATE_CONNECTOR.to_string(),
        ASSET_MANAGER.to_string(),
        VOTER_WHITELISTER.to_string(),
        FTSO_REGISTRY.to_string(),
        WNAT.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flare_registry_has_correct_address() {
        let registry = FlareContractRegistry::flare();
        let expected = Address::from_str(FLARE_REGISTRY_ADDRESS).unwrap();
        assert_eq!(registry.registry_address(), expected);
    }

    #[test]
    fn test_songbird_registry_has_correct_address() {
        let registry = FlareContractRegistry::songbird();
        let expected = Address::from_str(SONGBIRD_REGISTRY_ADDRESS).unwrap();
        assert_eq!(registry.registry_address(), expected);
    }

    #[test]
    fn test_default_contract_names_count() {
        let names = default_contract_names();
        assert_eq!(names.len(), 7);
        assert!(names.contains(&FTSO_MANAGER.to_string()));
        assert!(names.contains(&STATE_CONNECTOR.to_string()));
        assert!(names.contains(&ASSET_MANAGER.to_string()));
    }

    #[test]
    fn test_custom_registry() {
        let addr = Address::repeat_byte(0x42);
        let names = vec!["CustomContract".to_string()];
        let registry = FlareContractRegistry::custom(addr, names);
        assert_eq!(registry.registry_address(), addr);
        assert_eq!(registry.contract_names.len(), 1);
    }

    #[test]
    fn test_resolved_addresses_operations() {
        let mut map = HashMap::new();
        map.insert("FtsoManager".to_string(), Address::repeat_byte(0xAA));
        map.insert("StateConnector".to_string(), Address::repeat_byte(0xBB));

        let resolved = ResolvedAddresses { addresses: map };

        assert_eq!(resolved.len(), 2);
        assert!(!resolved.is_empty());
        assert_eq!(
            *resolved.get("FtsoManager").unwrap(),
            Address::repeat_byte(0xAA)
        );
        assert!(resolved.get("Nonexistent").is_none());
        assert_eq!(resolved.all_addresses().len(), 2);
    }

    #[test]
    fn test_empty_resolved_addresses() {
        let resolved = ResolvedAddresses {
            addresses: HashMap::new(),
        };
        assert!(resolved.is_empty());
        assert_eq!(resolved.len(), 0);
        assert!(resolved.all_addresses().is_empty());
    }
}
