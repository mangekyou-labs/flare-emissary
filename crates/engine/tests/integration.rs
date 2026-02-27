//! Integration tests for M2 engine components.
//!
//! Requires a running PostgreSQL database with `DATABASE_URL` env var set.
//! Run with:
//!
//! ```bash
//! DATABASE_URL="postgres://flare:flare@localhost:5432/flare_emissary" \
//!   cargo test -p flare-engine --test integration -- --ignored --nocapture
//! ```

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use flare_common::types::{Chain, DecodedEvent, EventType};
use flare_engine::analyzer::AddressAnalyzer;
use flare_engine::matcher::AlertMatcher;
use flare_engine::subscription::{
    CreateSubscriptionParams, SubscriptionService, UpdateSubscriptionParams,
};

// ============================================================
// Shared helpers
// ============================================================

/// Run migrations and clean up test data.
async fn setup(pool: &PgPool) {
    sqlx::migrate!("../../migrations").run(pool).await.unwrap();

    // Clean tables in dependency order
    sqlx::query("DELETE FROM notifications")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM alerts")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM subscriptions")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM notification_channels")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM indexed_events")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM monitored_addresses")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM users")
        .execute(pool)
        .await
        .unwrap();
}

/// Create a test user and return their ID.
async fn create_test_user(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, wallet_address) VALUES ($1, $2)")
        .bind(id)
        .bind(format!("0xtest_{}", id))
        .execute(pool)
        .await
        .unwrap();
    id
}

/// Create a monitored address and return its ID.
async fn create_monitored_address(pool: &PgPool, address: &str, chain: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO monitored_addresses (id, address, chain, address_type) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(address)
    .bind(chain)
    .bind("generic_contract")
    .execute(pool)
    .await
    .unwrap();
    id
}

/// Create a notification channel and return its ID.
async fn create_notification_channel(pool: &PgPool, user_id: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO notification_channels (id, user_id, channel_type, config) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(user_id)
    .bind("telegram")
    .bind(serde_json::json!({"chat_id": "12345"}))
    .execute(pool)
    .await
    .unwrap();
    id
}

// ============================================================
// 2.9: SubscriptionService CRUD
// ============================================================

#[sqlx::test]
#[ignore]
async fn test_subscription_create(pool: PgPool) {
    setup(&pool).await;
    let user_id = create_test_user(&pool).await;
    let addr_id = create_monitored_address(&pool, "0xsub_test", "flare").await;
    let chan_id = create_notification_channel(&pool, user_id).await;

    let params = CreateSubscriptionParams {
        address_id: addr_id,
        channel_id: chan_id,
        event_type: "price_epoch_finalized".to_string(),
        threshold_config: Some(serde_json::json!({"min_value": 0.5})),
    };

    let sub = SubscriptionService::create(&pool, user_id, &params)
        .await
        .unwrap();

    assert_eq!(sub.user_id, user_id);
    assert_eq!(sub.address_id, addr_id);
    assert_eq!(sub.channel_id, chan_id);
    assert!(sub.active);
    assert_eq!(sub.event_type, EventType::PriceEpochFinalized);
}

#[sqlx::test]
#[ignore]
async fn test_subscription_create_invalid_event_type(pool: PgPool) {
    setup(&pool).await;
    let user_id = create_test_user(&pool).await;
    let addr_id = create_monitored_address(&pool, "0xbad", "flare").await;
    let chan_id = create_notification_channel(&pool, user_id).await;

    let params = CreateSubscriptionParams {
        address_id: addr_id,
        channel_id: chan_id,
        event_type: "invalid_event_type".to_string(),
        threshold_config: None,
    };

    let result = SubscriptionService::create(&pool, user_id, &params).await;
    assert!(result.is_err(), "Should reject invalid event_type");
}

#[sqlx::test]
#[ignore]
async fn test_subscription_list_by_user(pool: PgPool) {
    setup(&pool).await;
    let user1 = create_test_user(&pool).await;
    let user2 = create_test_user(&pool).await;
    let addr_id = create_monitored_address(&pool, "0xlist_test", "flare").await;
    let chan_id1 = create_notification_channel(&pool, user1).await;
    let chan_id2 = create_notification_channel(&pool, user2).await;

    // Create 2 subs for user1, 1 for user2
    for _ in 0..2 {
        SubscriptionService::create(
            &pool,
            user1,
            &CreateSubscriptionParams {
                address_id: addr_id,
                channel_id: chan_id1,
                event_type: "generic_event".to_string(),
                threshold_config: None,
            },
        )
        .await
        .unwrap();
    }
    SubscriptionService::create(
        &pool,
        user2,
        &CreateSubscriptionParams {
            address_id: addr_id,
            channel_id: chan_id2,
            event_type: "generic_event".to_string(),
            threshold_config: None,
        },
    )
    .await
    .unwrap();

    let user1_subs = SubscriptionService::list_by_user(&pool, user1)
        .await
        .unwrap();
    let user2_subs = SubscriptionService::list_by_user(&pool, user2)
        .await
        .unwrap();

    assert_eq!(user1_subs.len(), 2);
    assert_eq!(user2_subs.len(), 1);
}

#[sqlx::test]
#[ignore]
async fn test_subscription_update(pool: PgPool) {
    setup(&pool).await;
    let user_id = create_test_user(&pool).await;
    let addr_id = create_monitored_address(&pool, "0xupdate_test", "flare").await;
    let chan_id = create_notification_channel(&pool, user_id).await;

    let sub = SubscriptionService::create(
        &pool,
        user_id,
        &CreateSubscriptionParams {
            address_id: addr_id,
            channel_id: chan_id,
            event_type: "generic_event".to_string(),
            threshold_config: None,
        },
    )
    .await
    .unwrap();

    assert!(sub.active);

    let updated = SubscriptionService::update(
        &pool,
        sub.id,
        user_id,
        &UpdateSubscriptionParams {
            active: Some(false),
            threshold_config: Some(serde_json::json!({"max_value": 999.0})),
            channel_id: None,
        },
    )
    .await
    .unwrap();

    assert!(!updated.active);
}

#[sqlx::test]
#[ignore]
async fn test_subscription_update_wrong_user(pool: PgPool) {
    setup(&pool).await;
    let owner = create_test_user(&pool).await;
    let other = create_test_user(&pool).await;
    let addr_id = create_monitored_address(&pool, "0xauth_test", "flare").await;
    let chan_id = create_notification_channel(&pool, owner).await;

    let sub = SubscriptionService::create(
        &pool,
        owner,
        &CreateSubscriptionParams {
            address_id: addr_id,
            channel_id: chan_id,
            event_type: "generic_event".to_string(),
            threshold_config: None,
        },
    )
    .await
    .unwrap();

    // Another user tries to update → should fail
    let result = SubscriptionService::update(
        &pool,
        sub.id,
        other,
        &UpdateSubscriptionParams {
            active: Some(false),
            threshold_config: None,
            channel_id: None,
        },
    )
    .await;

    assert!(
        result.is_err(),
        "Should not allow updating another user's subscription"
    );
}

#[sqlx::test]
#[ignore]
async fn test_subscription_delete(pool: PgPool) {
    setup(&pool).await;
    let user_id = create_test_user(&pool).await;
    let addr_id = create_monitored_address(&pool, "0xdelete_test", "flare").await;
    let chan_id = create_notification_channel(&pool, user_id).await;

    let sub = SubscriptionService::create(
        &pool,
        user_id,
        &CreateSubscriptionParams {
            address_id: addr_id,
            channel_id: chan_id,
            event_type: "generic_event".to_string(),
            threshold_config: None,
        },
    )
    .await
    .unwrap();

    let deleted = SubscriptionService::delete(&pool, sub.id, user_id)
        .await
        .unwrap();
    assert!(deleted);

    // Verify it's really gone
    let subs = SubscriptionService::list_by_user(&pool, user_id)
        .await
        .unwrap();
    assert!(subs.is_empty());
}

// ============================================================
// 2.10: AlertMatcher::find_matching_subscriptions
// ============================================================

#[sqlx::test]
#[ignore]
async fn test_matcher_finds_active_subscriptions(pool: PgPool) {
    setup(&pool).await;
    let user_id = create_test_user(&pool).await;
    let addr_id = create_monitored_address(&pool, "0xmatcher_addr", "flare").await;
    let chan_id = create_notification_channel(&pool, user_id).await;

    // Create matching subscription
    SubscriptionService::create(
        &pool,
        user_id,
        &CreateSubscriptionParams {
            address_id: addr_id,
            channel_id: chan_id,
            event_type: "price_epoch_finalized".to_string(),
            threshold_config: None,
        },
    )
    .await
    .unwrap();

    // Create non-matching subscription (different event type)
    SubscriptionService::create(
        &pool,
        user_id,
        &CreateSubscriptionParams {
            address_id: addr_id,
            channel_id: chan_id,
            event_type: "liquidation_started".to_string(),
            threshold_config: None,
        },
    )
    .await
    .unwrap();

    let event = DecodedEvent {
        tx_hash: "0xabc".to_string(),
        log_index: Some(0),
        block_number: 100,
        block_timestamp: Utc::now(),
        chain: Chain::Flare,
        address: "0xmatcher_addr".to_string(),
        event_type: EventType::PriceEpochFinalized,
        decoded_data: serde_json::json!({}),
    };

    let matcher = AlertMatcher::new();
    let matches = matcher
        .find_matching_subscriptions(&event, &pool)
        .await
        .unwrap();

    assert_eq!(
        matches.len(),
        1,
        "Should find exactly one matching subscription"
    );
    assert_eq!(matches[0].event_type, EventType::PriceEpochFinalized);
}

#[sqlx::test]
#[ignore]
async fn test_matcher_excludes_inactive_subscriptions(pool: PgPool) {
    setup(&pool).await;
    let user_id = create_test_user(&pool).await;
    let addr_id = create_monitored_address(&pool, "0xinactive_addr", "flare").await;
    let chan_id = create_notification_channel(&pool, user_id).await;

    // Create matching subscription then deactivate it
    let sub = SubscriptionService::create(
        &pool,
        user_id,
        &CreateSubscriptionParams {
            address_id: addr_id,
            channel_id: chan_id,
            event_type: "price_epoch_finalized".to_string(),
            threshold_config: None,
        },
    )
    .await
    .unwrap();

    SubscriptionService::update(
        &pool,
        sub.id,
        user_id,
        &UpdateSubscriptionParams {
            active: Some(false),
            threshold_config: None,
            channel_id: None,
        },
    )
    .await
    .unwrap();

    let event = DecodedEvent {
        tx_hash: "0xdef".to_string(),
        log_index: Some(0),
        block_number: 200,
        block_timestamp: Utc::now(),
        chain: Chain::Flare,
        address: "0xinactive_addr".to_string(),
        event_type: EventType::PriceEpochFinalized,
        decoded_data: serde_json::json!({}),
    };

    let matcher = AlertMatcher::new();
    let matches = matcher
        .find_matching_subscriptions(&event, &pool)
        .await
        .unwrap();

    assert!(
        matches.is_empty(),
        "Should not match inactive subscriptions"
    );
}

#[sqlx::test]
#[ignore]
async fn test_matcher_no_match_wrong_address(pool: PgPool) {
    setup(&pool).await;
    let user_id = create_test_user(&pool).await;
    let addr_id = create_monitored_address(&pool, "0xaddr_a", "flare").await;
    let chan_id = create_notification_channel(&pool, user_id).await;

    SubscriptionService::create(
        &pool,
        user_id,
        &CreateSubscriptionParams {
            address_id: addr_id,
            channel_id: chan_id,
            event_type: "price_epoch_finalized".to_string(),
            threshold_config: None,
        },
    )
    .await
    .unwrap();

    // Event from a DIFFERENT address
    let event = DecodedEvent {
        tx_hash: "0x999".to_string(),
        log_index: Some(0),
        block_number: 300,
        block_timestamp: Utc::now(),
        chain: Chain::Flare,
        address: "0xaddr_b_different".to_string(),
        event_type: EventType::PriceEpochFinalized,
        decoded_data: serde_json::json!({}),
    };

    let matcher = AlertMatcher::new();
    let matches = matcher
        .find_matching_subscriptions(&event, &pool)
        .await
        .unwrap();

    assert!(
        matches.is_empty(),
        "Should not match subscription on different address"
    );
}

// ============================================================
// 2.11: AddressAnalyzer::classify
// ============================================================

#[sqlx::test]
#[ignore]
async fn test_analyzer_classify_unknown_address(pool: PgPool) {
    setup(&pool).await;

    let result = AddressAnalyzer::classify("0xunknown_new", "flare", &pool)
        .await
        .unwrap();

    assert_eq!(result.address, "0xunknown_new");
    assert_eq!(
        result.address_type,
        flare_common::types::AddressType::GenericContract
    );
    assert!(!result.subscribable_events.is_empty());
    assert_eq!(result.label, "Smart Contract");

    // Verify it was inserted into DB
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM monitored_addresses WHERE address = '0xunknown_new' AND chain = 'flare'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count.0, 1, "Should have been inserted into DB");
}

#[sqlx::test]
#[ignore]
async fn test_analyzer_classify_cached_address(pool: PgPool) {
    setup(&pool).await;

    // Pre-insert a known FTSO provider address
    sqlx::query(
        "INSERT INTO monitored_addresses (address, chain, address_type) VALUES ($1, $2, $3)",
    )
    .bind("0xknown_ftso")
    .bind("flare")
    .bind("ftso_provider")
    .execute(&pool)
    .await
    .unwrap();

    let result = AddressAnalyzer::classify("0xknown_ftso", "flare", &pool)
        .await
        .unwrap();

    assert_eq!(
        result.address_type,
        flare_common::types::AddressType::FtsoProvider
    );
    assert_eq!(result.label, "FTSO Data Provider");
    assert_eq!(result.subscribable_events.len(), 3);
}

#[sqlx::test]
#[ignore]
async fn test_analyzer_idempotent_insert(pool: PgPool) {
    setup(&pool).await;

    // Classify twice — second should not error (ON CONFLICT DO NOTHING)
    AddressAnalyzer::classify("0xidempotent", "flare", &pool)
        .await
        .unwrap();
    AddressAnalyzer::classify("0xidempotent", "flare", &pool)
        .await
        .unwrap();

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM monitored_addresses WHERE address = '0xidempotent'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count.0, 1,
        "Should only have one row after duplicate classify"
    );
}
