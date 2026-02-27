//! Hysteresis engine — prevents alert fatigue from threshold oscillation.
//!
//! For FTSO price alerts, the price may oscillate around a threshold every 1.8s.
//! The hysteresis engine requires the threshold to be met for N consecutive blocks
//! before firing an alert.
//!
//! State is held in-memory per subscription ID. This is acceptable because:
//! - State is ephemeral — if the process restarts, hysteresis resets (conservative = good)
//! - No persistence overhead for high-frequency checks

use std::collections::HashMap;
use uuid::Uuid;

use flare_common::types::{Subscription, ThresholdConfig};

/// Default number of consecutive blocks required before an alert fires.
const DEFAULT_HYSTERESIS_BLOCKS: u64 = 1;

/// Per-subscription hysteresis tracking state.
#[derive(Debug, Clone)]
struct HysteresisState {
    /// Number of consecutive blocks where the threshold was met.
    consecutive_count: u64,
    /// Block number when the streak started.
    first_triggered_block: u64,
    /// Most recent block number in the streak.
    last_block: u64,
}

/// In-memory hysteresis engine.
pub struct HysteresisEngine {
    states: HashMap<Uuid, HysteresisState>,
}

impl HysteresisEngine {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// Check whether the hysteresis condition is satisfied for a subscription.
    ///
    /// - `subscription_id`: the subscription being evaluated
    /// - `threshold_met`: whether the threshold was met in the current block
    /// - `block_number`: the current block number
    /// - `subscription`: the subscription (for reading `hysteresis_blocks` config)
    ///
    /// Returns `true` only when the threshold has been met for N consecutive blocks.
    pub fn check(
        &mut self,
        subscription_id: Uuid,
        threshold_met: bool,
        block_number: u64,
        subscription: &Subscription,
    ) -> bool {
        let required = Self::required_blocks(subscription);

        if !threshold_met {
            // Threshold not met → reset state
            self.states.remove(&subscription_id);
            return false;
        }

        let state = self
            .states
            .entry(subscription_id)
            .or_insert(HysteresisState {
                consecutive_count: 0,
                first_triggered_block: block_number,
                last_block: 0,
            });

        // Check for consecutive blocks (allow same block for idempotency)
        if state.last_block == 0
            || block_number == state.last_block + 1
            || block_number == state.last_block
        {
            if block_number != state.last_block {
                state.consecutive_count += 1;
            }
            state.last_block = block_number;
        } else {
            // Gap in blocks → reset streak
            state.consecutive_count = 1;
            state.first_triggered_block = block_number;
            state.last_block = block_number;
        }

        if state.consecutive_count >= required {
            // Condition met — reset state so next alert needs fresh streak
            self.states.remove(&subscription_id);
            true
        } else {
            false
        }
    }

    /// Reset hysteresis state for a subscription.
    pub fn reset(&mut self, subscription_id: Uuid) {
        self.states.remove(&subscription_id);
    }

    /// Get the number of tracked subscriptions (for monitoring).
    pub fn tracked_count(&self) -> usize {
        self.states.len()
    }

    /// Extract the required consecutive blocks from subscription config.
    fn required_blocks(subscription: &Subscription) -> u64 {
        let config: ThresholdConfig =
            serde_json::from_value(subscription.threshold_config.clone()).unwrap_or_default();
        config
            .hysteresis_blocks
            .unwrap_or(DEFAULT_HYSTERESIS_BLOCKS)
    }
}

impl Default for HysteresisEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use flare_common::types::EventType;

    fn make_subscription(hysteresis_blocks: Option<u64>) -> Subscription {
        let mut config = serde_json::json!({});
        if let Some(n) = hysteresis_blocks {
            config["hysteresis_blocks"] = serde_json::json!(n);
        }
        Subscription {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            address_id: Uuid::new_v4(),
            channel_id: Uuid::new_v4(),
            event_type: EventType::PriceEpochFinalized,
            threshold_config: config,
            active: true,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_default_hysteresis_fires_immediately() {
        let mut engine = HysteresisEngine::new();
        let sub = make_subscription(None); // default = 1 block
        // First block with threshold met → should fire
        assert!(engine.check(sub.id, true, 100, &sub));
    }

    #[test]
    fn test_hysteresis_requires_consecutive_blocks() {
        let mut engine = HysteresisEngine::new();
        let sub = make_subscription(Some(3));

        // Block 100: threshold met → 1/3, not yet
        assert!(!engine.check(sub.id, true, 100, &sub));
        // Block 101: threshold met → 2/3, not yet
        assert!(!engine.check(sub.id, true, 101, &sub));
        // Block 102: threshold met → 3/3, fires!
        assert!(engine.check(sub.id, true, 102, &sub));
    }

    #[test]
    fn test_hysteresis_resets_on_threshold_not_met() {
        let mut engine = HysteresisEngine::new();
        let sub = make_subscription(Some(3));

        assert!(!engine.check(sub.id, true, 100, &sub));
        assert!(!engine.check(sub.id, true, 101, &sub));
        // Threshold not met → resets
        assert!(!engine.check(sub.id, false, 102, &sub));
        // Start over
        assert!(!engine.check(sub.id, true, 103, &sub));
        assert!(!engine.check(sub.id, true, 104, &sub));
        assert!(engine.check(sub.id, true, 105, &sub));
    }

    #[test]
    fn test_hysteresis_resets_on_block_gap() {
        let mut engine = HysteresisEngine::new();
        let sub = make_subscription(Some(3));

        assert!(!engine.check(sub.id, true, 100, &sub));
        assert!(!engine.check(sub.id, true, 101, &sub));
        // Gap: block 105 instead of 102 → resets streak
        assert!(!engine.check(sub.id, true, 105, &sub));
        assert!(!engine.check(sub.id, true, 106, &sub));
        assert!(engine.check(sub.id, true, 107, &sub));
    }

    #[test]
    fn test_hysteresis_state_cleared_after_fire() {
        let mut engine = HysteresisEngine::new();
        let sub = make_subscription(Some(2));

        assert!(!engine.check(sub.id, true, 100, &sub));
        assert!(engine.check(sub.id, true, 101, &sub));
        // State cleared after fire — need fresh 2 blocks
        assert!(!engine.check(sub.id, true, 102, &sub));
        assert!(engine.check(sub.id, true, 103, &sub));
    }

    #[test]
    fn test_independent_subscriptions() {
        let mut engine = HysteresisEngine::new();
        let sub1 = make_subscription(Some(2));
        let sub2 = make_subscription(Some(3));

        // Both at block 100
        assert!(!engine.check(sub1.id, true, 100, &sub1));
        assert!(!engine.check(sub2.id, true, 100, &sub2));

        // Block 101: sub1 fires (2/2), sub2 doesn't (2/3)
        assert!(engine.check(sub1.id, true, 101, &sub1));
        assert!(!engine.check(sub2.id, true, 101, &sub2));

        // Block 102: sub2 fires (3/3)
        assert!(engine.check(sub2.id, true, 102, &sub2));
    }

    #[test]
    fn test_reset() {
        let mut engine = HysteresisEngine::new();
        let sub = make_subscription(Some(3));

        assert!(!engine.check(sub.id, true, 100, &sub));
        assert!(!engine.check(sub.id, true, 101, &sub));
        assert_eq!(engine.tracked_count(), 1);

        engine.reset(sub.id);
        assert_eq!(engine.tracked_count(), 0);

        // Must start over
        assert!(!engine.check(sub.id, true, 102, &sub));
    }
}
