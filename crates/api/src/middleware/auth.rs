//! JWT authentication middleware and helpers.
//!
//! Provides JWT encoding/decoding plus an `AuthUser` Axum extractor
//! that validates the Authorization header on protected routes.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use flare_common::error::AppError;

use crate::state::AppState;

/// JWT claims stored in the token.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject — the user's UUID
    pub sub: String,
    /// Expiration time (UNIX timestamp)
    pub exp: i64,
    /// Issued at (UNIX timestamp)
    pub iat: i64,
}

/// Authenticated user extracted from JWT token.
///
/// Use as an Axum extractor on protected routes:
/// ```ignore
/// async fn handler(auth: AuthUser) -> impl IntoResponse {
///     // auth.user_id is the authenticated user's UUID
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub claims: Claims,
}

/// Encode a JWT token for a user.
pub fn encode_jwt(user_id: Uuid, secret: &str, expiry_hours: u64) -> Result<String, AppError> {
    let now = Utc::now();
    let exp = now + Duration::hours(expiry_hours as i64);

    let claims = Claims {
        sub: user_id.to_string(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Auth(format!("Failed to encode JWT: {}", e)))?;

    Ok(token)
}

/// Decode and validate a JWT token.
pub fn decode_jwt(token: &str, secret: &str) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| AppError::Auth(format!("Invalid token: {}", e)))?;

    Ok(token_data.claims)
}

/// Axum `FromRequestParts` implementation for `AuthUser`.
///
/// Extracts and validates the JWT from the `Authorization: Bearer <token>` header.
/// Also supports API key authentication via the `X-API-Key` header.
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let secret = state.config.jwt_secret.clone();
        let pool = state.pool.clone();

        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let api_key_header = parts
            .headers
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        async move {
            // Try JWT Bearer token first
            if let Some(auth) = auth_header
                && let Some(token) = auth.strip_prefix("Bearer ")
            {
                let claims = decode_jwt(token, &secret)?;
                let user_id = Uuid::parse_str(&claims.sub)
                    .map_err(|_| AppError::Auth("Invalid user ID in token".to_string()))?;
                return Ok(AuthUser { user_id, claims });
            }

            // Try API key
            if let Some(api_key) = api_key_header {
                let user_id: Option<(Uuid,)> =
                    sqlx::query_as("SELECT id FROM users WHERE api_key = $1")
                        .bind(&api_key)
                        .fetch_optional(&pool)
                        .await
                        .map_err(|e| AppError::Internal(format!("DB error: {}", e)))?;

                if let Some((id,)) = user_id {
                    let now = Utc::now();
                    let claims = Claims {
                        sub: id.to_string(),
                        exp: (now + Duration::hours(24)).timestamp(),
                        iat: now.timestamp(),
                    };
                    return Ok(AuthUser {
                        user_id: id,
                        claims,
                    });
                }
            }

            Err(AppError::Auth(
                "Missing or invalid Authorization header. Use 'Bearer <JWT>' or 'X-API-Key: <key>'"
                    .to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-secret-key-for-unit-tests";

    #[test]
    fn test_encode_decode_jwt() {
        let user_id = Uuid::new_v4();
        let token = encode_jwt(user_id, TEST_SECRET, 24).unwrap();
        let claims = decode_jwt(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.sub, user_id.to_string());
        assert!(claims.exp > Utc::now().timestamp());
    }

    #[test]
    fn test_invalid_secret_rejected() {
        let user_id = Uuid::new_v4();
        let token = encode_jwt(user_id, TEST_SECRET, 24).unwrap();
        let result = decode_jwt(&token, "wrong-secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_expired_jwt_rejected() {
        let user_id = Uuid::new_v4();
        // Create a token that expired 1 hour ago
        let now = Utc::now();
        let exp = now - Duration::hours(1);
        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp(),
            iat: (now - Duration::hours(2)).timestamp(),
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();

        let result = decode_jwt(&token, TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn test_garbage_token_rejected() {
        let result = decode_jwt("not.a.valid.jwt", TEST_SECRET);
        assert!(result.is_err());
    }
}
