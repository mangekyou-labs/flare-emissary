//! Integration tests for BlockPoller persistence and reorg rollback logic.
//!
//! These tests require a running PostgreSQL database and the `DATABASE_URL`
//! environment variable to be set. Run with:
//!
//! ```bash
//! DATABASE_URL="postgresql://..." cargo test -p flare-indexer --test integration -- --ignored --nocapture
//! ```

use chrono::Utc;
use sqlx::PgPool;

use flare_common::types::{Chain, DecodedEvent, EventType};
use flare_indexer::poller::BlockPoller;

/// Create a BlockPoller connected to the test database.
async fn setup(pool: &PgPool) -> BlockPoller {
    // Run migrations
    sqlx::migrate!("../../migrations").run(pool).await.unwrap();

    // Clean up any leftover data from previous runs
    sqlx::query("DELETE FROM indexed_events WHERE chain = 'flare'")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM indexer_state WHERE chain = 'flare'")
        .execute(pool)
        .await
        .unwrap();

    BlockPoller::new(
        "http://localhost:9650/ext/bc/C/rpc".to_string(), // not used in these tests
        1500,
        Chain::Flare,
        pool.clone(),
        10,
    )
}

fn make_event(block_number: u64, log_index: u64, event_type: EventType) -> DecodedEvent {
    DecodedEvent {
        tx_hash: format!("0x{:064x}", block_number * 1000 + log_index),
        log_index: Some(log_index),
        block_number,
        block_timestamp: Utc::now(),
        chain: Chain::Flare,
        address: "0x0000000000000000000000000000000000001234".to_string(),
        event_type,
        decoded_data: serde_json::json!({"test": true}),
    }
}

#[sqlx::test]
#[ignore] // Requires DATABASE_URL — run explicitly with --ignored
async fn test_persist_events_inserts_correctly(pool: PgPool) {
    let poller = setup(&pool).await;

    let events = vec![
        make_event(100, 0, EventType::PriceEpochFinalized),
        make_event(100, 1, EventType::VotePowerChanged),
    ];

    poller.persist_events(&events).await.unwrap();

    // Verify events were inserted
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM indexed_events WHERE block_number = 100 AND chain = 'flare'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(count.0, 2, "Expected 2 events inserted");

    // Verify event types
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT event_type FROM indexed_events WHERE block_number = 100 ORDER BY log_index"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows[0].0, "price_epoch_finalized");
    assert_eq!(rows[1].0, "vote_power_changed");
}

#[sqlx::test]
#[ignore]
async fn test_persist_events_deduplication(pool: PgPool) {
    let poller = setup(&pool).await;

    let events = vec![make_event(200, 0, EventType::CollateralDeposited)];

    // Insert once
    poller.persist_events(&events).await.unwrap();
    // Insert again — should not error (ON CONFLICT DO NOTHING)
    poller.persist_events(&events).await.unwrap();

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM indexed_events WHERE block_number = 200"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(count.0, 1, "Duplicate insert should be ignored");
}

#[sqlx::test]
#[ignore]
async fn test_update_and_get_indexer_state(pool: PgPool) {
    let poller = setup(&pool).await;

    // Initially no state
    let initial = poller.get_last_indexed_block().await.unwrap();
    assert!(initial.is_none(), "Expected no initial state");

    // Set block 500
    poller.update_indexer_state(500).await.unwrap();
    let block = poller.get_last_indexed_block().await.unwrap();
    assert_eq!(block, Some(500));

    // Update to block 1000
    poller.update_indexer_state(1000).await.unwrap();
    let block = poller.get_last_indexed_block().await.unwrap();
    assert_eq!(block, Some(1000));
}

#[sqlx::test]
#[ignore]
async fn test_rollback_marks_events_reorged(pool: PgPool) {
    let poller = setup(&pool).await;

    // Insert events across multiple blocks
    let events = vec![
        make_event(300, 0, EventType::AttestationRequested),
        make_event(301, 0, EventType::AttestationProved),
        make_event(302, 0, EventType::RoundFinalized),
        make_event(303, 0, EventType::MintingExecuted),
    ];
    poller.persist_events(&events).await.unwrap();

    // Rollback from block 302
    poller.rollback_events_from(302).await.unwrap();

    // Blocks 300, 301 should NOT be reorged
    let safe: Vec<(i64, bool)> = sqlx::query_as(
        "SELECT block_number, is_reorged FROM indexed_events WHERE block_number < 302 ORDER BY block_number"
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    for (block, reorged) in &safe {
        assert!(!reorged, "Block {} should not be marked as reorged", block);
    }

    // Blocks 302, 303 SHOULD be reorged
    let reorged: Vec<(i64, bool)> = sqlx::query_as(
        "SELECT block_number, is_reorged FROM indexed_events WHERE block_number >= 302 ORDER BY block_number"
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(reorged.len(), 2);
    for (block, is_reorged) in &reorged {
        assert!(is_reorged, "Block {} should be marked as reorged", block);
    }
}
