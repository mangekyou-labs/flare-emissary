//! Cooldown engine — Redis-backed per-subscription cooldown timers.
//!
//! After an alert fires, the subscription enters a cooldown period during which
//! no further alerts are generated. This prevents flooding the user's channels
//! with duplicate notifications for the same condition.
//!
//! Uses Redis `SET NX EX` for atomic check-and-set with automatic TTL expiry.

use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use uuid::Uuid;

use flare_common::types::{Subscription, ThresholdConfig};

/// Default cooldown duration in seconds (5 minutes).
const DEFAULT_COOLDOWN_SECONDS: u64 = 300;

/// Redis-backed cooldown engine.
pub struct CooldownEngine;

impl CooldownEngine {
    pub fn new() -> Self {
        Self
    }

    /// Check if a subscription is in cooldown, and if not, set the cooldown.
    ///
    /// Returns `true` if the subscription is NOT in cooldown (alert should proceed).
    /// Returns `false` if the subscription IS in cooldown (alert should be suppressed).
    ///
    /// Uses Redis `SET key value NX EX ttl` for atomic check-and-set:
    /// - NX = only set if key doesn't exist
    /// - EX = set TTL in seconds
    pub async fn check_and_set(
        &self,
        redis: &mut ConnectionManager,
        subscription_id: Uuid,
        subscription: &Subscription,
    ) -> anyhow::Result<bool> {
        let cooldown_secs = Self::cooldown_seconds(subscription);
        let key = format!("subscription:cooldown:{}", subscription_id);

        // SET key "1" NX EX cooldown_secs
        // Returns Some("OK") if key was set (not in cooldown)
        // Returns None if key already exists (in cooldown)
        let result: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(cooldown_secs)
            .query_async(redis)
            .await?;

        let allowed = result.is_some();

        if !allowed {
            tracing::debug!(
                subscription_id = %subscription_id,
                cooldown_secs,
                "Alert suppressed — subscription in cooldown"
            );
        }

        Ok(allowed)
    }

    /// Clear the cooldown for a subscription (e.g., when subscription is updated).
    pub async fn clear(
        &self,
        redis: &mut ConnectionManager,
        subscription_id: Uuid,
    ) -> anyhow::Result<()> {
        let key = format!("subscription:cooldown:{}", subscription_id);
        redis.del::<_, ()>(&key).await?;
        Ok(())
    }

    /// Extract cooldown seconds from subscription config.
    fn cooldown_seconds(subscription: &Subscription) -> u64 {
        let config: ThresholdConfig =
            serde_json::from_value(subscription.threshold_config.clone()).unwrap_or_default();
        config.cooldown_seconds.unwrap_or(DEFAULT_COOLDOWN_SECONDS)
    }
}

impl Default for CooldownEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cooldown_seconds_default() {
        let sub = Subscription {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            address_id: Uuid::new_v4(),
            channel_id: Uuid::new_v4(),
            event_type: flare_common::types::EventType::PriceEpochFinalized,
            threshold_config: serde_json::json!({}),
            active: true,
            created_at: chrono::Utc::now(),
        };
        assert_eq!(CooldownEngine::cooldown_seconds(&sub), 300);
    }

    #[test]
    fn test_cooldown_seconds_custom() {
        let sub = Subscription {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            address_id: Uuid::new_v4(),
            channel_id: Uuid::new_v4(),
            event_type: flare_common::types::EventType::PriceEpochFinalized,
            threshold_config: serde_json::json!({"cooldown_seconds": 60}),
            active: true,
            created_at: chrono::Utc::now(),
        };
        assert_eq!(CooldownEngine::cooldown_seconds(&sub), 60);
    }
}
