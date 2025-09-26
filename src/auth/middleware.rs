use axum::{
    async_trait,
    extract::{FromRequestParts, State},
    http::{request::Parts, HeaderValue},
    RequestPartsExt,
};
use std::sync::Arc;

use crate::{
    auth::{
        scopes::{Scope, ScopeSet},
        tokens::{hash_token, TokenGenerator},
    },
    error::{AppError, AppResult},
    models::{AppState, User, AccessToken},
};

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user: User,
    pub scopes: ScopeSet,
    pub client_id: String,
}

#[derive(Clone, Debug)]
pub struct AuthClient {
    pub client_id: String,
    pub scopes: ScopeSet,
}

// For endpoints that require user authentication
#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let state = parts
            .extensions
            .get::<Arc<AppState>>()
            .ok_or_else(|| AppError::Other(anyhow::anyhow!("AppState not found in request extensions")))?;

        let token_info = extract_and_validate_token(parts, state).await?;

        let user_id = token_info.user_id
            .ok_or_else(|| AppError::Unauthorized("User authentication required".to_string()))?;

        // Look up user
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE id = ?1"
        )
        .bind(&user_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

        Ok(AuthUser {
            user,
            scopes: token_info.scopes,
            client_id: token_info.client_id,
        })
    }
}

// For endpoints that accept both user and client authentication
#[async_trait]
impl<S> FromRequestParts<S> for AuthClient
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let state = parts
            .extensions
            .get::<Arc<AppState>>()
            .ok_or_else(|| AppError::Other(anyhow::anyhow!("AppState not found in request extensions")))?;

        let token_info = extract_and_validate_token(parts, state).await?;

        Ok(AuthClient {
            client_id: token_info.client_id,
            scopes: token_info.scopes,
        })
    }
}

#[derive(Debug)]
struct TokenInfo {
    pub client_id: String,
    pub user_id: Option<String>,
    pub scopes: ScopeSet,
}

async fn extract_and_validate_token(
    parts: &Parts,
    state: &Arc<AppState>,
) -> Result<TokenInfo, AppError> {
    // Extract Authorization header
    let auth_header = parts
        .headers
        .get("Authorization")
        .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".to_string()))?;

    let auth_str = auth_header
        .to_str()
        .map_err(|_| AppError::Unauthorized("Invalid Authorization header".to_string()))?;

    // Check for Bearer token
    if !auth_str.starts_with("Bearer ") {
        return Err(AppError::Unauthorized("Bearer token required".to_string()));
    }

    let token = &auth_str[7..]; // Remove "Bearer " prefix

    // Validate token
    let token_generator = TokenGenerator::new()
        .map_err(|_| AppError::Other(anyhow::anyhow!("Token generator initialization failed")))?;

    let claims = token_generator
        .validate_access_token(token)
        .map_err(|_| AppError::Unauthorized("Invalid or expired token".to_string()))?;

    // Check if token exists in database and is not revoked
    let stored_token = sqlx::query_as::<_, AccessToken>(
        "SELECT * FROM access_tokens WHERE token_hash = ?1 AND revoked = FALSE"
    )
    .bind(hash_token(&claims.jti))
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::Other(anyhow::anyhow!("Database error: {}", e)))?
    .ok_or_else(|| AppError::Unauthorized("Token not found or revoked".to_string()))?;

    // Parse scopes
    let scopes = ScopeSet::from_string(&claims.scopes)
        .map_err(|_| AppError::Unauthorized("Invalid token scopes".to_string()))?;

    Ok(TokenInfo {
        client_id: claims.client_id,
        user_id: stored_token.user_id,
        scopes,
    })
}

// Middleware for requiring specific scopes
pub struct RequireScope(pub Scope);

#[async_trait]
impl<S> FromRequestParts<S> for RequireScope
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // This is just a marker type, actual scope validation happens in the handler
        // or through the AuthUser/AuthClient extractors
        Err(AppError::Other(anyhow::anyhow!("RequireScope should not be used directly")))
    }
}

// Helper function to check if user has required scope
pub fn check_scope(auth: &AuthUser, required_scope: &Scope) -> AppResult<()> {
    if auth.scopes.contains(required_scope) {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!("Required scope: {}", required_scope.as_str())))
    }
}

// Helper function to check if client has required scope
pub fn check_client_scope(auth: &AuthClient, required_scope: &Scope) -> AppResult<()> {
    if auth.scopes.contains(required_scope) {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!("Required scope: {}", required_scope.as_str())))
    }
}

// Convenience macro for requiring specific scopes in handlers
#[macro_export]
macro_rules! require_scope {
    ($auth:expr, $scope:expr) => {
        $crate::auth::middleware::check_scope($auth, $scope)?;
    };
}

#[macro_export]
macro_rules! require_client_scope {
    ($auth:expr, $scope:expr) => {
        $crate::auth::middleware::check_client_scope($auth, $scope)?;
    };
}