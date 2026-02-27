//! Address analysis routes.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};

use flare_common::error::AppError;
use flare_engine::analyzer::{AddressAnalyzer, AddressClassification};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/addresses/{address}/analyze", get(analyze_address))
}

/// GET /api/addresses/:address/analyze — Classify an address and return subscribable events.
///
/// This endpoint is public (no auth required) to allow discovery before sign-up.
async fn analyze_address(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<AddressClassification>, AppError> {
    // Default to "flare" chain; could be extended with query param
    let classification = AddressAnalyzer::classify(&address, "flare", &state.pool).await?;
    Ok(Json(classification))
}
