//! Event processing pipeline.
//!
//! Receives decoded events from the indexer and:
//! 1. Translates them into human-readable notification payloads
//! 2. Matches against active subscriptions (via `AlertMatcher`)
//! 3. Evaluates hysteresis and cooldown (via `HysteresisEngine` + `CooldownEngine`)
//! 4. Creates `Alert` + `Notification` records for qualifying events

use chrono::Utc;
use redis::aio::ConnectionManager;
use sqlx::PgPool;
use uuid::Uuid;

use flare_common::types::{DecodedEvent, DeliveryStatus, EventType, NotificationPayload, Severity};

use crate::cooldown::CooldownEngine;
use crate::hysteresis::HysteresisEngine;
use crate::matcher::AlertMatcher;

/// Central event processor that orchestrates the alert pipeline.
pub struct EventProcessor {
    matcher: AlertMatcher,
    hysteresis: HysteresisEngine,
    cooldown: CooldownEngine,
}

impl EventProcessor {
    pub fn new() -> Self {
        Self {
            matcher: AlertMatcher::new(),
            hysteresis: HysteresisEngine::new(),
            cooldown: CooldownEngine::new(),
        }
    }

    /// Process a decoded event through the full alert pipeline.
    ///
    /// Steps:
    /// 1. Translate event → human-readable payload
    /// 2. Find matching subscriptions
    /// 3. For each match: evaluate threshold → hysteresis → cooldown
    /// 4. Create Alert + Notification records for qualifying matches
    pub async fn process_event(
        &mut self,
        event: &DecodedEvent,
        pool: &PgPool,
        redis: &mut ConnectionManager,
    ) -> anyhow::Result<u32> {
        let payload = Self::translate_event(event);

        // Find all active subscriptions matching this event's address + type
        let subscriptions = self
            .matcher
            .find_matching_subscriptions(event, pool)
            .await?;

        if subscriptions.is_empty() {
            return Ok(0);
        }

        let mut alerts_created = 0u32;

        for sub in &subscriptions {
            // Evaluate threshold
            if !AlertMatcher::evaluate_threshold(sub, event) {
                continue;
            }

            // Evaluate hysteresis (must meet threshold for N consecutive blocks)
            if !self.hysteresis.check(sub.id, true, event.block_number, sub) {
                continue;
            }

            // Evaluate cooldown
            if !self.cooldown.check_and_set(redis, sub.id, sub).await? {
                continue;
            }

            // All checks passed — create alert + notification
            let alert_id = Uuid::new_v4();
            let notification_id = Uuid::new_v4();
            let now = Utc::now();

            // Insert alert
            sqlx::query(
                r#"
                INSERT INTO alerts (id, subscription_id, event_id, severity, message, triggered_at)
                VALUES ($1, $2, (
                    SELECT id FROM indexed_events
                    WHERE tx_hash = $3 AND COALESCE(log_index, -1) = COALESCE($4, -1)
                    LIMIT 1
                ), $5, $6, $7)
                "#,
            )
            .bind(alert_id)
            .bind(sub.id)
            .bind(&event.tx_hash)
            .bind(event.log_index.map(|i| i as i64))
            .bind(payload.severity.to_string())
            .bind(&payload.body)
            .bind(now)
            .execute(pool)
            .await?;

            // Insert notification (pending delivery)
            sqlx::query(
                r#"
                INSERT INTO notifications (id, alert_id, channel_id, status, created_at)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(notification_id)
            .bind(alert_id)
            .bind(sub.channel_id)
            .bind(DeliveryStatus::Pending.to_string())
            .bind(now)
            .execute(pool)
            .await?;

            tracing::info!(
                alert_id = %alert_id,
                subscription_id = %sub.id,
                event_type = %event.event_type,
                "Alert created"
            );

            alerts_created += 1;
        }

        Ok(alerts_created)
    }

    /// Translate a decoded event into a human-readable notification payload.
    pub fn translate_event(event: &DecodedEvent) -> NotificationPayload {
        let (title, body, severity) = match event.event_type {
            // FTSO events
            EventType::PriceEpochFinalized => {
                let epoch = event
                    .decoded_data
                    .get("epoch_id")
                    .and_then(|v| v.as_u64())
                    .map(|e| format!("#{}", e))
                    .unwrap_or_else(|| "unknown".to_string());
                let feed = event
                    .decoded_data
                    .get("feed_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let price = event
                    .decoded_data
                    .get("price")
                    .and_then(|v| v.as_f64())
                    .map(|p| format!("{:.6}", p))
                    .unwrap_or_else(|| "N/A".to_string());

                (
                    "FTSO Price Epoch Finalized".to_string(),
                    format!(
                        "Price epoch {} finalized for feed {}: price = {}",
                        epoch, feed, price
                    ),
                    Severity::Info,
                )
            }
            EventType::VotePowerChanged => {
                let provider = event
                    .decoded_data
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let old = event
                    .decoded_data
                    .get("old_vote_power")
                    .and_then(|v| v.as_str())
                    .unwrap_or("N/A");
                let new = event
                    .decoded_data
                    .get("new_vote_power")
                    .and_then(|v| v.as_str())
                    .unwrap_or("N/A");

                (
                    "FTSO Vote Power Changed".to_string(),
                    format!(
                        "Provider {} vote power changed: {} → {}",
                        provider, old, new
                    ),
                    Severity::Warning,
                )
            }
            EventType::RewardEpochStarted => {
                let epoch = event
                    .decoded_data
                    .get("reward_epoch_id")
                    .and_then(|v| v.as_u64())
                    .map(|e| format!("#{}", e))
                    .unwrap_or_else(|| "unknown".to_string());

                (
                    "Reward Epoch Started".to_string(),
                    format!("New reward epoch {} has started", epoch),
                    Severity::Info,
                )
            }

            // FDC events
            EventType::AttestationRequested => {
                let source = event
                    .decoded_data
                    .get("source_chain")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                (
                    "Attestation Requested".to_string(),
                    format!(
                        "New attestation request from {} at block {}",
                        source, event.block_number
                    ),
                    Severity::Info,
                )
            }
            EventType::AttestationProved => (
                "Attestation Proved".to_string(),
                format!(
                    "Attestation proved in tx {} at block {}",
                    &event.tx_hash, event.block_number
                ),
                Severity::Info,
            ),
            EventType::RoundFinalized => {
                let round = event
                    .decoded_data
                    .get("round_id")
                    .and_then(|v| v.as_u64())
                    .map(|r| format!("#{}", r))
                    .unwrap_or_else(|| "unknown".to_string());

                (
                    "FDC Round Finalized".to_string(),
                    format!("FDC round {} finalized", round),
                    Severity::Info,
                )
            }

            // FAsset events
            EventType::CollateralDeposited => {
                let amount = event
                    .decoded_data
                    .get("amount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("N/A");

                (
                    "Collateral Deposited".to_string(),
                    format!(
                        "Collateral deposited: {} at address {}",
                        amount, event.address
                    ),
                    Severity::Info,
                )
            }
            EventType::CollateralWithdrawn => {
                let amount = event
                    .decoded_data
                    .get("amount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("N/A");

                (
                    "Collateral Withdrawn".to_string(),
                    format!(
                        "Collateral withdrawn: {} from address {}",
                        amount, event.address
                    ),
                    Severity::Warning,
                )
            }
            EventType::MintingExecuted => {
                let amount = event
                    .decoded_data
                    .get("amount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("N/A");

                (
                    "FAsset Minting Executed".to_string(),
                    format!("Minting executed: {} FAssets at {}", amount, event.address),
                    Severity::Info,
                )
            }
            EventType::RedemptionRequested => (
                "FAsset Redemption Requested".to_string(),
                format!(
                    "Redemption requested at address {} in block {}",
                    event.address, event.block_number
                ),
                Severity::Warning,
            ),
            EventType::LiquidationStarted => (
                "⚠️ Liquidation Started".to_string(),
                format!(
                    "CRITICAL: Liquidation started for agent {} at block {}!",
                    event.address, event.block_number
                ),
                Severity::Critical,
            ),

            // Generic
            EventType::GenericEvent => (
                "Contract Event".to_string(),
                format!(
                    "Event detected on {} at block {}",
                    event.address, event.block_number
                ),
                Severity::Info,
            ),
        };

        NotificationPayload {
            title,
            body,
            severity,
            metadata: event.decoded_data.clone(),
        }
    }
}

impl Default for EventProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use flare_common::types::Chain;

    fn make_event(event_type: EventType, data: serde_json::Value) -> DecodedEvent {
        DecodedEvent {
            tx_hash: "0xabc123".to_string(),
            log_index: Some(0),
            block_number: 1000,
            block_timestamp: Utc::now(),
            chain: Chain::Flare,
            address: "0x1234".to_string(),
            event_type,
            decoded_data: data,
        }
    }

    #[test]
    fn test_translate_price_epoch_finalized() {
        let event = make_event(
            EventType::PriceEpochFinalized,
            serde_json::json!({
                "epoch_id": 42,
                "feed_id": "FLR/USD",
                "price": 0.0245
            }),
        );
        let payload = EventProcessor::translate_event(&event);
        assert_eq!(payload.title, "FTSO Price Epoch Finalized");
        assert!(payload.body.contains("#42"));
        assert!(payload.body.contains("FLR/USD"));
        assert!(payload.body.contains("0.024500"));
        assert_eq!(payload.severity, Severity::Info);
    }

    #[test]
    fn test_translate_vote_power_changed() {
        let event = make_event(
            EventType::VotePowerChanged,
            serde_json::json!({
                "provider": "0xABC",
                "old_vote_power": "1000000",
                "new_vote_power": "950000"
            }),
        );
        let payload = EventProcessor::translate_event(&event);
        assert_eq!(payload.title, "FTSO Vote Power Changed");
        assert!(payload.body.contains("0xABC"));
        assert!(payload.body.contains("1000000"));
        assert!(payload.body.contains("950000"));
        assert_eq!(payload.severity, Severity::Warning);
    }

    #[test]
    fn test_translate_liquidation_started() {
        let event = make_event(EventType::LiquidationStarted, serde_json::json!({}));
        let payload = EventProcessor::translate_event(&event);
        assert!(payload.title.contains("Liquidation"));
        assert_eq!(payload.severity, Severity::Critical);
    }

    #[test]
    fn test_translate_collateral_deposited() {
        let event = make_event(
            EventType::CollateralDeposited,
            serde_json::json!({ "amount": "500000000000000000" }),
        );
        let payload = EventProcessor::translate_event(&event);
        assert_eq!(payload.title, "Collateral Deposited");
        assert!(payload.body.contains("500000000000000000"));
    }

    #[test]
    fn test_translate_generic_event() {
        let event = make_event(EventType::GenericEvent, serde_json::json!({}));
        let payload = EventProcessor::translate_event(&event);
        assert_eq!(payload.title, "Contract Event");
        assert_eq!(payload.severity, Severity::Info);
    }
}
