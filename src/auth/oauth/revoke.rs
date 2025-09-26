use axum::{
    extract::State,
    http::StatusCode,
};
use axum_extra::extract::Form;
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    auth::tokens::{hash_token, TokenGenerator},
    error::{AppError, AppResult},
    models::{AppState, RevokeRequest},
};

// POST /oauth/revoke - Token revocation
#[utoipa::path(
    post,
    path = "/oauth/revoke",
    request_body = RevokeRequest,
    responses(
        (status = 200, description = "Token revoked successfully"),
        (status = 400, description = "Invalid token"),
    )
)]
pub async fn revoke(
    State(state): State<Arc<AppState>>,
    Form(req): Form<RevokeRequest>,
) -> AppResult<StatusCode> {
    let token_generator = TokenGenerator::new()
        .map_err(|_| AppError::Other(anyhow::anyhow!("Token generator initialization failed")))?;

    let mut revoked = false;

    // Try to revoke as access token
    if let Ok(claims) = token_generator.validate_access_token(&req.token) {
        let rows_affected = sqlx::query(
            "UPDATE access_tokens SET revoked = TRUE WHERE token_hash = ?1"
        )
        .bind(hash_token(&claims.jti))
        .execute(&state.pool)
        .await?
        .rows_affected();

        if rows_affected > 0 {
            revoked = true;
        }
    }

    // Try to revoke as refresh token
    if !revoked {
        if let Ok(claims) = token_generator.validate_refresh_token(&req.token) {
            let rows_affected = sqlx::query(
                "UPDATE refresh_tokens SET revoked = TRUE WHERE token_hash = ?1"
            )
            .bind(hash_token(&claims.jti))
            .execute(&state.pool)
            .await?
            .rows_affected();

            if rows_affected > 0 {
                revoked = true;

                // Also revoke all associated access tokens for this refresh token
                // This is a security best practice
                sqlx::query(
                    "UPDATE access_tokens SET revoked = TRUE WHERE client_id = ?1 AND user_id = ?2"
                )
                .bind(&claims.client_id)
                .bind(&claims.sub)
                .execute(&state.pool)
                .await?;
            }
        }
    }

    // According to RFC 7009, the revocation endpoint should return 200 OK
    // even if the token was invalid or already revoked
    Ok(StatusCode::OK)
}