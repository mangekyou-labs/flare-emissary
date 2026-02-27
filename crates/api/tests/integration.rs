//! Integration tests for API routes.
//!
//! Uses `tower::ServiceExt` to test Axum routes without a real HTTP server.
//! Requires a running PostgreSQL database.
//!
//! ```bash
//! DATABASE_URL="postgres://flare:flare@localhost:5432/flare_emissary" \
//!   cargo test -p flare-api --test integration -- --ignored --nocapture
//! ```

use axum::body::Body;
use axum::http::{Request, StatusCode};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use flare_api::routes::create_router;
use flare_api::state::AppState;
use flare_common::config::AppConfig;

// ============================================================
// Helpers
// ============================================================

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

/// Create a test AppConfig with a specific JWT secret.
fn test_config() -> AppConfig {
    AppConfig {
        flare_rpc_url: "http://unused".to_string(),
        flare_rpc_fallback_url: None,
        database_url: "unused".to_string(),
        redis_url: "redis://localhost:6379".to_string(),
        indexer_poll_interval_ms: 1500,
        indexer_reorg_window: 10,
        jwt_secret: "test-jwt-secret-for-integration-tests".to_string(),
        jwt_expiry_hours: 24,
        telegram_bot_token: None,
        discord_bot_token: None,
        resend_api_key: None,
        email_from: None,
        db_max_connections: 5,
    }
}

/// Create a test user and return a JWT token for them.
async fn create_user_with_token(pool: &PgPool) -> (Uuid, String) {
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, wallet_address) VALUES ($1, $2)")
        .bind(user_id)
        .bind(format!("0xtest_{}", user_id))
        .execute(pool)
        .await
        .unwrap();

    let config = test_config();
    let token = flare_api::middleware::auth::encode_jwt(
        user_id,
        &config.jwt_secret,
        config.jwt_expiry_hours,
    )
    .unwrap();

    (user_id, token)
}

/// Build an AppState for testing (uses real DB but dummy Redis).
async fn build_test_state(pool: PgPool) -> AppState {
    let config = test_config();
    // Create a Redis connection for the test (may fail if Redis isn't running)
    let redis = redis::Client::open(config.redis_url.as_str())
        .unwrap()
        .get_connection_manager()
        .await
        .unwrap();
    AppState::new(pool, redis, config)
}

// ============================================================
// 2.13: API Route Tests
// ============================================================

#[sqlx::test]
#[ignore]
async fn test_health_endpoint(pool: PgPool) {
    setup(&pool).await;
    let state = build_test_state(pool).await;
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["service"], "flare-emissary-api");
}

#[sqlx::test]
#[ignore]
async fn test_subscription_crud_via_api(pool: PgPool) {
    setup(&pool).await;
    let (user_id, token) = create_user_with_token(&pool).await;

    // Create supporting entities
    let addr_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO monitored_addresses (id, address, chain, address_type) VALUES ($1, $2, $3, $4)",
    )
    .bind(addr_id)
    .bind("0xapi_test")
    .bind("flare")
    .bind("generic_contract")
    .execute(&pool)
    .await
    .unwrap();

    let chan_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO notification_channels (id, user_id, channel_type, config) VALUES ($1, $2, $3, $4)",
    )
    .bind(chan_id)
    .bind(user_id)
    .bind("telegram")
    .bind(serde_json::json!({"chat_id": "12345"}))
    .execute(&pool)
    .await
    .unwrap();

    let state = build_test_state(pool).await;

    // 1. Create subscription
    let app = create_router(state.clone());
    let create_body = serde_json::json!({
        "address_id": addr_id,
        "channel_id": chan_id,
        "event_type": "generic_event",
        "threshold_config": {"min_value": 10.0}
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/subscriptions")
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let sub_id = created["id"].as_str().unwrap();
    assert_eq!(created["active"], true);

    // 2. List subscriptions
    let app = create_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/subscriptions")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.len(), 1);

    // 3. Update subscription (deactivate)
    let app = create_router(state.clone());
    let update_body = serde_json::json!({"active": false});

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/subscriptions/{}", sub_id))
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&update_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["active"], false);

    // 4. Delete subscription
    let app = create_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/subscriptions/{}", sub_id))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[sqlx::test]
#[ignore]
async fn test_subscription_requires_auth(pool: PgPool) {
    setup(&pool).await;
    let state = build_test_state(pool).await;
    let app = create_router(state);

    // No auth header → 401
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/subscriptions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
#[ignore]
async fn test_address_analyze_endpoint(pool: PgPool) {
    setup(&pool).await;
    let state = build_test_state(pool).await;
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/addresses/0xnew_contract/analyze")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["address"], "0xnew_contract");
    assert_eq!(json["label"], "Smart Contract");
    assert!(!json["subscribable_events"].as_array().unwrap().is_empty());
}

#[sqlx::test]
#[ignore]
async fn test_invalid_jwt_rejected(pool: PgPool) {
    setup(&pool).await;
    let state = build_test_state(pool).await;
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/subscriptions")
                .header("authorization", "Bearer invalid.jwt.token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
