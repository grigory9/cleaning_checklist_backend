use axum::{
    extract::State,
    Json,
};
use axum_extra::extract::Form;
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    auth::{
        scopes::ScopeSet,
        tokens::{hash_token, TokenGenerator},
    },
    error::{AppError, AppResult},
    models::{AppState, AccessToken, RefreshToken, IntrospectRequest, IntrospectResponse},
};

// POST /oauth/introspect - Token introspection
#[utoipa::path(
    post,
    path = "/oauth/introspect",
    request_body = IntrospectRequest,
    responses(
        (status = 200, description = "Token introspection response", body = IntrospectResponse),
    )
)]
pub async fn introspect(
    State(state): State<Arc<AppState>>,
    Form(req): Form<IntrospectRequest>,
) -> AppResult<Json<IntrospectResponse>> {
    // For simplicity, we'll accept any authenticated client
    // In production, you might want to authenticate the client making the introspection request

    let token_generator = TokenGenerator::new()
        .map_err(|_| AppError::Other(anyhow::anyhow!("Token generator initialization failed")))?;

    // Try to parse as access token first
    if let Ok(claims) = token_generator.validate_access_token(&req.token) {
        // Check if token exists in database and is not revoked
        let stored_token = sqlx::query_as::<_, AccessToken>(
            "SELECT * FROM access_tokens WHERE token_hash = ?1 AND revoked = FALSE"
        )
        .bind(hash_token(&claims.jti))
        .fetch_optional(&state.pool)
        .await?;

        if let Some(token) = stored_token {
            // Parse scopes
            let scopes = ScopeSet::from_string(&claims.scopes)
                .map_err(|_| AppError::BadRequest("Invalid scopes".to_string()))?;

            return Ok(Json(IntrospectResponse {
                active: true,
                scope: Some(scopes.to_string()),
                client_id: Some(claims.client_id),
                username: token.user_id.clone(),
                exp: Some(claims.exp),
                iat: Some(claims.iat),
                sub: Some(claims.sub),
            }));
        }
    }

    // Try to parse as refresh token
    if let Ok(claims) = token_generator.validate_refresh_token(&req.token) {
        // Check if token exists in database and is not revoked
        let stored_token = sqlx::query_as::<_, RefreshToken>(
            "SELECT * FROM refresh_tokens WHERE token_hash = ?1 AND revoked = FALSE"
        )
        .bind(hash_token(&claims.jti))
        .fetch_optional(&state.pool)
        .await?;

        if let Some(token) = stored_token {
            // Parse scopes
            let scopes = ScopeSet::from_string(&claims.scopes)
                .map_err(|_| AppError::BadRequest("Invalid scopes".to_string()))?;

            return Ok(Json(IntrospectResponse {
                active: true,
                scope: Some(scopes.to_string()),
                client_id: Some(claims.client_id),
                username: token.user_id.clone(),
                exp: Some(claims.exp),
                iat: Some(claims.iat),
                sub: Some(claims.sub),
            }));
        }
    }

    // Token is invalid, expired, or revoked
    Ok(Json(IntrospectResponse {
        active: false,
        scope: None,
        client_id: None,
        username: None,
        exp: None,
        iat: None,
        sub: None,
    }))
}