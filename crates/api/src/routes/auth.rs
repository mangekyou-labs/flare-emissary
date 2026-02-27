//! Authentication routes — SIWE verification and API key generation.

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use flare_common::error::AppError;
use flare_common::types::User;

use crate::middleware::auth::{AuthUser, encode_jwt};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/siwe", post(siwe_login))
        .route("/api/auth/api-keys", post(generate_api_key))
}

/// Request body for SIWE login.
#[derive(Debug, Deserialize)]
pub struct SiweLoginRequest {
    /// The SIWE message string
    pub message: String,
    /// The wallet's signature of the message
    pub signature: String,
}

/// Response for successful login.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: Uuid,
    pub wallet_address: String,
}

/// Response for API key generation.
#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    pub api_key: String,
}

/// POST /api/auth/siwe — Verify SIWE message + signature, upsert user, return JWT.
async fn siwe_login(
    State(state): State<AppState>,
    Json(req): Json<SiweLoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    // Parse and verify the SIWE message
    let message: siwe::Message = req
        .message
        .parse()
        .map_err(|e| AppError::Validation(format!("Invalid SIWE message: {}", e)))?;

    // Decode the hex signature
    let sig_bytes = hex_decode(&req.signature)?;

    // Verify the signature (with default verification options — no domain/nonce check)
    let opts = siwe::VerificationOpts::default();
    message
        .verify(&sig_bytes, &opts)
        .await
        .map_err(|e| AppError::Auth(format!("Signature verification failed: {}", e)))?;

    // Extract wallet address from the verified message
    let wallet_address = format!(
        "0x{}",
        message
            .address
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    );

    // Upsert user in the database
    let user: User = sqlx::query_as(
        r#"
        INSERT INTO users (wallet_address)
        VALUES ($1)
        ON CONFLICT (wallet_address) DO UPDATE SET updated_at = NOW()
        RETURNING *
        "#,
    )
    .bind(&wallet_address)
    .fetch_one(&state.pool)
    .await?;

    // Generate JWT
    let token = encode_jwt(
        user.id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )?;

    tracing::info!(
        user_id = %user.id,
        wallet = %wallet_address,
        "User authenticated via SIWE"
    );

    Ok(Json(LoginResponse {
        token,
        user_id: user.id,
        wallet_address,
    }))
}

/// POST /api/auth/api-keys — Generate a new API key for the authenticated user.
async fn generate_api_key(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<ApiKeyResponse>, AppError> {
    // Generate a random API key
    let api_key = format!("fe_{}", Uuid::new_v4().to_string().replace('-', ""));

    // Store in the database
    sqlx::query("UPDATE users SET api_key = $1, updated_at = NOW() WHERE id = $2")
        .bind(&api_key)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await?;

    tracing::info!(
        user_id = %auth.user_id,
        "API key generated"
    );

    Ok(Json(ApiKeyResponse { api_key }))
}

/// Decode a hex-encoded string (with or without 0x prefix) into bytes.
fn hex_decode(hex: &str) -> Result<Vec<u8>, AppError> {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    if !hex.len().is_multiple_of(2) {
        return Err(AppError::Validation(
            "Hex string must have even length".to_string(),
        ));
    }
    let bytes: Result<Vec<u8>, _> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect();
    bytes.map_err(|e| AppError::Validation(format!("Invalid hex signature: {}", e)))
}
