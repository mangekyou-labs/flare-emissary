pub mod addresses;
pub mod auth;
pub mod health;
pub mod subscriptions;

use axum::Router;

use crate::state::AppState;

/// Build the complete API router with all routes.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .merge(health::router())
        .merge(auth::router())
        .merge(subscriptions::router())
        .merge(addresses::router())
        .with_state(state)
}
