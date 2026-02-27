//! Subscription CRUD routes.

use axum::extract::{Path, State};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use uuid::Uuid;

use flare_common::error::AppError;
use flare_common::types::Subscription;
use flare_engine::subscription::{
    CreateSubscriptionParams, SubscriptionService, UpdateSubscriptionParams,
};

use crate::middleware::auth::AuthUser;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/subscriptions", post(create_subscription))
        .route("/api/subscriptions", get(list_subscriptions))
        .route("/api/subscriptions/{id}", patch(update_subscription))
        .route("/api/subscriptions/{id}", delete(delete_subscription))
}

/// POST /api/subscriptions — Create a new alert subscription.
async fn create_subscription(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(params): Json<CreateSubscriptionParams>,
) -> Result<Json<Subscription>, AppError> {
    let subscription = SubscriptionService::create(&state.pool, auth.user_id, &params).await?;
    Ok(Json(subscription))
}

/// GET /api/subscriptions — List all subscriptions for the authenticated user.
async fn list_subscriptions(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<Subscription>>, AppError> {
    let subscriptions = SubscriptionService::list_by_user(&state.pool, auth.user_id).await?;
    Ok(Json(subscriptions))
}

/// PATCH /api/subscriptions/:id — Update a subscription.
async fn update_subscription(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(params): Json<UpdateSubscriptionParams>,
) -> Result<Json<Subscription>, AppError> {
    let subscription = SubscriptionService::update(&state.pool, id, auth.user_id, &params).await?;
    Ok(Json(subscription))
}

/// DELETE /api/subscriptions/:id — Delete a subscription.
async fn delete_subscription(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let deleted = SubscriptionService::delete(&state.pool, id, auth.user_id).await?;
    if deleted {
        Ok(Json(serde_json::json!({"deleted": true})))
    } else {
        Err(AppError::NotFound(format!("Subscription {} not found", id)))
    }
}
