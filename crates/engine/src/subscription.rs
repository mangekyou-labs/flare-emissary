//! Subscription service — CRUD operations for user alert subscriptions.
//!
//! Subscriptions link a user to a monitored address + event type + notification channel,
//! with optional threshold configuration for conditional alerting.

use sqlx::PgPool;
use uuid::Uuid;

use flare_common::error::AppError;
use flare_common::types::Subscription;

/// Service layer for subscription CRUD operations.
pub struct SubscriptionService;

/// Parameters for creating a new subscription.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateSubscriptionParams {
    pub address_id: Uuid,
    pub channel_id: Uuid,
    pub event_type: String,
    pub threshold_config: Option<serde_json::Value>,
}

/// Parameters for updating an existing subscription.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UpdateSubscriptionParams {
    pub active: Option<bool>,
    pub threshold_config: Option<serde_json::Value>,
    pub channel_id: Option<Uuid>,
}

impl SubscriptionService {
    /// Valid event type strings, matching `EventType::Display` output.
    const VALID_EVENT_TYPES: &[&str] = &[
        "price_epoch_finalized",
        "vote_power_changed",
        "reward_epoch_started",
        "attestation_requested",
        "attestation_proved",
        "round_finalized",
        "collateral_deposited",
        "collateral_withdrawn",
        "minting_executed",
        "redemption_requested",
        "liquidation_started",
        "generic_event",
    ];

    /// Create a new subscription for a user.
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        params: &CreateSubscriptionParams,
    ) -> Result<Subscription, AppError> {
        // Validate event_type against known types
        if !Self::VALID_EVENT_TYPES.contains(&params.event_type.as_str()) {
            return Err(AppError::Validation(format!(
                "Invalid event_type '{}'. Valid types: {}",
                params.event_type,
                Self::VALID_EVENT_TYPES.join(", ")
            )));
        }

        let id = Uuid::new_v4();
        let threshold = params
            .threshold_config
            .clone()
            .unwrap_or(serde_json::json!({}));

        let sub: Subscription = sqlx::query_as(
            r#"
            INSERT INTO subscriptions (id, user_id, address_id, channel_id, event_type, threshold_config, active)
            VALUES ($1, $2, $3, $4, $5, $6, true)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(user_id)
        .bind(params.address_id)
        .bind(params.channel_id)
        .bind(&params.event_type)
        .bind(&threshold)
        .fetch_one(pool)
        .await?;

        tracing::info!(
            subscription_id = %sub.id,
            user_id = %user_id,
            event_type = %params.event_type,
            "Subscription created"
        );

        Ok(sub)
    }

    /// List all subscriptions for a user.
    pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Subscription>, AppError> {
        let subs: Vec<Subscription> = sqlx::query_as(
            "SELECT * FROM subscriptions WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(subs)
    }

    /// Get a single subscription by ID.
    pub async fn get(pool: &PgPool, subscription_id: Uuid) -> Result<Subscription, AppError> {
        let sub: Subscription = sqlx::query_as("SELECT * FROM subscriptions WHERE id = $1")
            .bind(subscription_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Subscription {} not found", subscription_id))
            })?;

        Ok(sub)
    }

    /// Update a subscription's active status and/or threshold config.
    pub async fn update(
        pool: &PgPool,
        subscription_id: Uuid,
        user_id: Uuid,
        params: &UpdateSubscriptionParams,
    ) -> Result<Subscription, AppError> {
        // Verify ownership
        let existing = Self::get(pool, subscription_id).await?;
        if existing.user_id != user_id {
            return Err(AppError::Auth(
                "Not authorized to update this subscription".to_string(),
            ));
        }

        let active = params.active.unwrap_or(existing.active);
        let threshold = params
            .threshold_config
            .clone()
            .unwrap_or(existing.threshold_config);
        let channel_id = params.channel_id.unwrap_or(existing.channel_id);

        let sub: Subscription = sqlx::query_as(
            r#"
            UPDATE subscriptions
            SET active = $1, threshold_config = $2, channel_id = $3
            WHERE id = $4
            RETURNING *
            "#,
        )
        .bind(active)
        .bind(&threshold)
        .bind(channel_id)
        .bind(subscription_id)
        .fetch_one(pool)
        .await?;

        tracing::info!(
            subscription_id = %subscription_id,
            active,
            "Subscription updated"
        );

        Ok(sub)
    }

    /// Delete a subscription. Returns true if it was deleted.
    pub async fn delete(
        pool: &PgPool,
        subscription_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM subscriptions WHERE id = $1 AND user_id = $2")
            .bind(subscription_id)
            .bind(user_id)
            .execute(pool)
            .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            tracing::info!(subscription_id = %subscription_id, "Subscription deleted");
        }

        Ok(deleted)
    }

    /// Find all active subscriptions matching an address and event type.
    /// Used by the alert matcher during event processing.
    pub async fn find_active_by_address_and_event(
        pool: &PgPool,
        address: &str,
        event_type: &str,
    ) -> Result<Vec<Subscription>, AppError> {
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
        .bind(address)
        .bind(event_type)
        .fetch_all(pool)
        .await?;

        Ok(subs)
    }
}
