//! Alert matcher — evaluates incoming events against user subscriptions.
//!
//! For each incoming event:
//! 1. Load active subscriptions filtering by address + event_type
//! 2. Evaluate threshold_config against event decoded_data
//! 3. Return qualifying subscriptions for further hysteresis/cooldown evaluation

use sqlx::PgPool;

use flare_common::types::{DecodedEvent, Subscription, ThresholdConfig};

/// Alert matcher that evaluates events against subscriptions.
pub struct AlertMatcher;

impl AlertMatcher {
    pub fn new() -> Self {
        Self
    }

    /// Find all active subscriptions whose monitored address and event type
    /// match the incoming event.
    pub async fn find_matching_subscriptions(
        &self,
        event: &DecodedEvent,
        pool: &PgPool,
    ) -> anyhow::Result<Vec<Subscription>> {
        let subs: Vec<Subscription> = sqlx::query_as(
            r#"
            SELECT s.*
            FROM subscriptions s
            JOIN monitored_addresses ma ON s.address_id = ma.id
            WHERE ma.address = $1
              AND s.event_type = $2
              AND s.active = true
            "#,
        )
        .bind(&event.address)
        .bind(event.event_type.to_string())
        .fetch_all(pool)
        .await?;

        Ok(subs)
    }

    /// Evaluate whether an event meets a subscription's threshold criteria.
    ///
    /// Threshold config fields:
    /// - `min_value`: alert if event value drops below this
    /// - `max_value`: alert if event value rises above this
    /// - `deviation_pct`: alert if percentage deviation exceeds this
    ///
    /// If no threshold fields are set, the subscription always matches
    /// (useful for "notify on any event of this type" subscriptions).
    pub fn evaluate_threshold(subscription: &Subscription, event: &DecodedEvent) -> bool {
        let config: ThresholdConfig =
            serde_json::from_value(subscription.threshold_config.clone()).unwrap_or_default();

        // Extract the primary numeric value from the event data
        let event_value = Self::extract_value(&event.decoded_data);

        // If no thresholds set, always match
        if config.min_value.is_none()
            && config.max_value.is_none()
            && config.deviation_pct.is_none()
        {
            return true;
        }

        // If we need a value but can't extract one, don't match
        let value = match event_value {
            Some(v) => v,
            None => return false,
        };

        // Check min_value (alert when value drops BELOW min)
        if let Some(min) = config.min_value
            && value < min
        {
            return true;
        }

        // Check max_value (alert when value rises ABOVE max)
        if let Some(max) = config.max_value
            && value > max
        {
            return true;
        }

        // Check deviation_pct (alert when deviation exceeds threshold)
        if let Some(deviation_threshold) = config.deviation_pct
            && let Some(baseline) = event.decoded_data.get("baseline").and_then(|v| v.as_f64())
            && baseline != 0.0
        {
            let deviation = ((value - baseline) / baseline).abs() * 100.0;
            if deviation >= deviation_threshold {
                return true;
            }
        }

        false
    }

    /// Extract the primary numeric value from event decoded data.
    ///
    /// Looks for common field names: `value`, `price`, `amount`, `cr`.
    fn extract_value(data: &serde_json::Value) -> Option<f64> {
        for key in &["value", "price", "amount", "cr", "vote_power"] {
            if let Some(v) = data.get(key) {
                if let Some(f) = v.as_f64() {
                    return Some(f);
                }
                // Try parsing string values (e.g., big numbers as strings)
                if let Some(s) = v.as_str()
                    && let Ok(f) = s.parse::<f64>()
                {
                    return Some(f);
                }
            }
        }
        None
    }
}

impl Default for AlertMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use flare_common::types::{Chain, EventType};
    use uuid::Uuid;

    fn make_event(data: serde_json::Value) -> DecodedEvent {
        DecodedEvent {
            tx_hash: "0x123".to_string(),
            log_index: Some(0),
            block_number: 100,
            block_timestamp: Utc::now(),
            chain: Chain::Flare,
            address: "0xtest".to_string(),
            event_type: EventType::PriceEpochFinalized,
            decoded_data: data,
        }
    }

    fn make_subscription(threshold: serde_json::Value) -> Subscription {
        Subscription {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            address_id: Uuid::new_v4(),
            channel_id: Uuid::new_v4(),
            event_type: EventType::PriceEpochFinalized,
            threshold_config: threshold,
            active: true,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_no_threshold_always_matches() {
        let event = make_event(serde_json::json!({"price": 100.0}));
        let sub = make_subscription(serde_json::json!({}));
        assert!(AlertMatcher::evaluate_threshold(&sub, &event));
    }

    #[test]
    fn test_price_above_max_triggers() {
        let event = make_event(serde_json::json!({"price": 150.0}));
        let sub = make_subscription(serde_json::json!({"max_value": 100.0}));
        assert!(AlertMatcher::evaluate_threshold(&sub, &event));
    }

    #[test]
    fn test_price_below_max_does_not_trigger() {
        let event = make_event(serde_json::json!({"price": 50.0}));
        let sub = make_subscription(serde_json::json!({"max_value": 100.0}));
        assert!(!AlertMatcher::evaluate_threshold(&sub, &event));
    }

    #[test]
    fn test_price_below_min_triggers() {
        let event = make_event(serde_json::json!({"price": 0.5}));
        let sub = make_subscription(serde_json::json!({"min_value": 1.0}));
        assert!(AlertMatcher::evaluate_threshold(&sub, &event));
    }

    #[test]
    fn test_price_above_min_does_not_trigger() {
        let event = make_event(serde_json::json!({"price": 5.0}));
        let sub = make_subscription(serde_json::json!({"min_value": 1.0}));
        assert!(!AlertMatcher::evaluate_threshold(&sub, &event));
    }

    #[test]
    fn test_deviation_pct_triggers() {
        let event = make_event(serde_json::json!({
            "price": 120.0,
            "baseline": 100.0
        }));
        let sub = make_subscription(serde_json::json!({"deviation_pct": 15.0}));
        // 20% deviation >= 15% threshold → triggers
        assert!(AlertMatcher::evaluate_threshold(&sub, &event));
    }

    #[test]
    fn test_deviation_pct_does_not_trigger() {
        let event = make_event(serde_json::json!({
            "price": 105.0,
            "baseline": 100.0
        }));
        let sub = make_subscription(serde_json::json!({"deviation_pct": 15.0}));
        // 5% deviation < 15% threshold → no trigger
        assert!(!AlertMatcher::evaluate_threshold(&sub, &event));
    }

    #[test]
    fn test_string_value_parsed() {
        let event = make_event(serde_json::json!({"amount": "500"}));
        let sub = make_subscription(serde_json::json!({"max_value": 100.0}));
        // "500" parsed as 500.0 > 100.0 → triggers
        assert!(AlertMatcher::evaluate_threshold(&sub, &event));
    }

    #[test]
    fn test_no_value_with_threshold_does_not_match() {
        let event = make_event(serde_json::json!({"some_field": "abc"}));
        let sub = make_subscription(serde_json::json!({"min_value": 1.0}));
        // Can't extract value → doesn't match
        assert!(!AlertMatcher::evaluate_threshold(&sub, &event));
    }
}
