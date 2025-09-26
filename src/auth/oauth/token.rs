use axum::{
    extract::State,
    Json,
};
use axum_extra::extract::Form;
use chrono::{Duration, Utc};
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    auth::{
        password::verify_password,
        scopes::ScopeSet,
        tokens::{hash_token, generate_code_challenge, TokenGenerator},
    },
    error::{AppError, AppResult},
    models::{AppState, AuthorizationCode, OAuthClient, TokenResponse, RefreshToken},
};

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    // Authorization code grant
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_verifier: Option<String>, // PKCE
    // Refresh token grant
    pub refresh_token: Option<String>,
    // Client credentials grant
    pub scope: Option<String>,
}

// POST /oauth/token - Token endpoint
#[utoipa::path(
    post,
    path = "/oauth/token",
    request_body = TokenRequest,
    responses(
        (status = 200, description = "Token response", body = TokenResponse),
        (status = 400, description = "Token error", body = TokenErrorResponse),
    )
)]
pub async fn token(
    State(state): State<Arc<AppState>>,
    Form(req): Form<TokenRequest>,
) -> AppResult<Json<TokenResponse>> {
    match req.grant_type.as_str() {
        "authorization_code" => handle_authorization_code_grant(state, req).await,
        "refresh_token" => handle_refresh_token_grant(state, req).await,
        "client_credentials" => handle_client_credentials_grant(state, req).await,
        _ => Err(AppError::BadRequest("unsupported_grant_type".to_string())),
    }
}

async fn handle_authorization_code_grant(
    state: Arc<AppState>,
    req: TokenRequest,
) -> AppResult<Json<TokenResponse>> {
    let code = req.code.ok_or_else(|| AppError::BadRequest("code is required".to_string()))?;
    let redirect_uri = req.redirect_uri.ok_or_else(|| AppError::BadRequest("redirect_uri is required".to_string()))?;

    // Authenticate client
    let _client = authenticate_client(&state, &req.client_id, req.client_secret.as_deref()).await?;

    // Fetch and validate authorization code
    let auth_code = sqlx::query_as::<_, AuthorizationCode>(
        "SELECT * FROM authorization_codes WHERE code = ?1 AND client_id = ?2"
    )
    .bind(&code)
    .bind(&req.client_id)
    .fetch_optional(&state.pool)
    .await?;

    let auth_code = auth_code.ok_or_else(|| AppError::BadRequest("invalid_grant".to_string()))?;

    // Check if code expired
    if auth_code.expires_at < Utc::now() {
        // Delete expired code
        sqlx::query("DELETE FROM authorization_codes WHERE code = ?1")
            .bind(&code)
            .execute(&state.pool)
            .await?;
        return Err(AppError::BadRequest("invalid_grant".to_string()));
    }

    // Validate redirect URI
    if auth_code.redirect_uri != redirect_uri {
        return Err(AppError::BadRequest("invalid_grant".to_string()));
    }

    // Validate PKCE if present
    if let Some(ref challenge) = auth_code.code_challenge {
        let verifier = req.code_verifier.ok_or_else(|| AppError::BadRequest("code_verifier is required".to_string()))?;
        let method = auth_code.code_challenge_method.as_deref().unwrap_or("plain");

        let expected_challenge = match method {
            "S256" => generate_code_challenge(&verifier),
            "plain" => verifier,
            _ => return Err(AppError::BadRequest("invalid_request".to_string())),
        };

        if challenge != &expected_challenge {
            return Err(AppError::BadRequest("invalid_grant".to_string()));
        }
    }

    // Parse scopes
    let scopes = ScopeSet::from_string(&auth_code.scopes)
        .map_err(|_| AppError::BadRequest("invalid_scope".to_string()))?;

    // Generate tokens
    let token_generator = TokenGenerator::new()
        .map_err(|_| AppError::Other(anyhow::anyhow!("Token generator initialization failed")))?;

    let (access_token, access_jti) = token_generator
        .generate_access_token(Some(&auth_code.user_id), &auth_code.client_id, &scopes, 60)
        .map_err(|e| AppError::Other(anyhow::anyhow!("Access token generation failed: {}", e)))?;

    let (refresh_token, refresh_jti) = token_generator
        .generate_refresh_token(Some(&auth_code.user_id), &auth_code.client_id, &scopes, 30)
        .map_err(|e| AppError::Other(anyhow::anyhow!("Refresh token generation failed: {}", e)))?;

    let access_expires_at = Utc::now() + Duration::minutes(60);
    let refresh_expires_at = Utc::now() + Duration::days(30);

    // Store tokens in database
    sqlx::query(
        r#"INSERT INTO access_tokens
           (token_hash, client_id, user_id, scopes, expires_at, created_at, revoked)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
    )
    .bind(hash_token(&access_jti))
    .bind(&auth_code.client_id)
    .bind(&auth_code.user_id)
    .bind(&auth_code.scopes)
    .bind(&access_expires_at)
    .bind(&Utc::now())
    .bind(false)
    .execute(&state.pool)
    .await?;

    sqlx::query(
        r#"INSERT INTO refresh_tokens
           (token_hash, client_id, user_id, scopes, expires_at, created_at, revoked)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
    )
    .bind(hash_token(&refresh_jti))
    .bind(&auth_code.client_id)
    .bind(&auth_code.user_id)
    .bind(&auth_code.scopes)
    .bind(&refresh_expires_at)
    .bind(&Utc::now())
    .bind(false)
    .execute(&state.pool)
    .await?;

    // Delete used authorization code
    sqlx::query("DELETE FROM authorization_codes WHERE code = ?1")
        .bind(&code)
        .execute(&state.pool)
        .await?;

    Ok(Json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: 3600, // 60 minutes in seconds
        refresh_token: Some(refresh_token),
        scope: scopes.to_string(),
    }))
}

async fn handle_refresh_token_grant(
    state: Arc<AppState>,
    req: TokenRequest,
) -> AppResult<Json<TokenResponse>> {
    let refresh_token = req.refresh_token.ok_or_else(|| AppError::BadRequest("refresh_token is required".to_string()))?;

    // Authenticate client
    let _client = authenticate_client(&state, &req.client_id, req.client_secret.as_deref()).await?;

    // Validate refresh token
    let token_generator = TokenGenerator::new()
        .map_err(|_| AppError::Other(anyhow::anyhow!("Token generator initialization failed")))?;

    let refresh_claims = token_generator
        .validate_refresh_token(&refresh_token)
        .map_err(|_| AppError::BadRequest("invalid_grant".to_string()))?;

    // Verify client matches
    if refresh_claims.client_id != req.client_id {
        return Err(AppError::BadRequest("invalid_grant".to_string()));
    }

    // Check if refresh token exists in database and is not revoked
    let stored_token = sqlx::query_as::<_, RefreshToken>(
        "SELECT * FROM refresh_tokens WHERE token_hash = ?1 AND revoked = FALSE"
    )
    .bind(hash_token(&refresh_claims.jti))
    .fetch_optional(&state.pool)
    .await?;

    let _stored_token = stored_token.ok_or_else(|| AppError::BadRequest("invalid_grant".to_string()))?;

    // Parse scopes
    let scopes = ScopeSet::from_string(&refresh_claims.scopes)
        .map_err(|_| AppError::BadRequest("invalid_scope".to_string()))?;

    // Generate new access token
    let user_id = if refresh_claims.sub == refresh_claims.client_id {
        None // Client credentials grant
    } else {
        Some(refresh_claims.sub.as_str())
    };

    let (new_access_token, access_jti) = token_generator
        .generate_access_token(user_id, &refresh_claims.client_id, &scopes, 60)
        .map_err(|e| AppError::Other(anyhow::anyhow!("Access token generation failed: {}", e)))?;

    let access_expires_at = Utc::now() + Duration::minutes(60);

    // Store new access token
    sqlx::query(
        r#"INSERT INTO access_tokens
           (token_hash, client_id, user_id, scopes, expires_at, created_at, revoked)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
    )
    .bind(hash_token(&access_jti))
    .bind(&refresh_claims.client_id)
    .bind(user_id)
    .bind(&refresh_claims.scopes)
    .bind(&access_expires_at)
    .bind(&Utc::now())
    .bind(false)
    .execute(&state.pool)
    .await?;

    Ok(Json(TokenResponse {
        access_token: new_access_token,
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token: Some(refresh_token), // Return the same refresh token
        scope: scopes.to_string(),
    }))
}

async fn handle_client_credentials_grant(
    state: Arc<AppState>,
    req: TokenRequest,
) -> AppResult<Json<TokenResponse>> {
    // Authenticate client
    let client = authenticate_client(&state, &req.client_id, req.client_secret.as_deref()).await?;

    // Validate requested scopes against client's allowed scopes
    let client_scopes: Vec<String> = serde_json::from_str(&client.scopes)
        .map_err(|_| AppError::BadRequest("Invalid client scopes".to_string()))?;

    let requested_scopes = req.scope.as_deref().unwrap_or("");
    let scopes = if requested_scopes.is_empty() {
        ScopeSet::from_vec(client_scopes.iter().filter_map(|s| s.parse().ok()).collect())
    } else {
        let requested = ScopeSet::from_string(requested_scopes)
            .map_err(|_| AppError::BadRequest("invalid_scope".to_string()))?;

        // Verify all requested scopes are allowed for this client
        for scope in requested.iter() {
            if !client_scopes.contains(&scope.as_str().to_string()) {
                return Err(AppError::BadRequest("invalid_scope".to_string()));
            }
        }
        requested
    };

    // Generate access token (no refresh token for client credentials)
    let token_generator = TokenGenerator::new()
        .map_err(|_| AppError::Other(anyhow::anyhow!("Token generator initialization failed")))?;

    let (access_token, access_jti) = token_generator
        .generate_access_token(None, &req.client_id, &scopes, 60)
        .map_err(|e| AppError::Other(anyhow::anyhow!("Access token generation failed: {}", e)))?;

    let access_expires_at = Utc::now() + Duration::minutes(60);

    // Store access token
    sqlx::query(
        r#"INSERT INTO access_tokens
           (token_hash, client_id, user_id, scopes, expires_at, created_at, revoked)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
    )
    .bind(hash_token(&access_jti))
    .bind(&req.client_id)
    .bind::<Option<String>>(None) // No user for client credentials
    .bind(scopes.to_json_array().to_string())
    .bind(&access_expires_at)
    .bind(&Utc::now())
    .bind(false)
    .execute(&state.pool)
    .await?;

    Ok(Json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token: None, // No refresh token for client credentials
        scope: scopes.to_string(),
    }))
}

async fn authenticate_client(
    state: &Arc<AppState>,
    client_id: &str,
    client_secret: Option<&str>,
) -> AppResult<OAuthClient> {
    let client = sqlx::query_as::<_, OAuthClient>(
        "SELECT * FROM oauth_clients WHERE client_id = ?1"
    )
    .bind(client_id)
    .fetch_optional(&state.pool)
    .await?;

    let client = client.ok_or_else(|| AppError::Unauthorized("Invalid client".to_string()))?;

    // For public clients (PKCE), no client secret is required
    if client.is_public {
        return Ok(client);
    }

    // For confidential clients, verify client secret
    let provided_secret = client_secret.ok_or_else(|| AppError::Unauthorized("Client authentication required".to_string()))?;

    let is_valid = verify_password(provided_secret, &client.client_secret_hash)
        .map_err(|e| AppError::Other(anyhow::anyhow!("Client authentication failed: {}", e)))?;

    if !is_valid {
        return Err(AppError::Unauthorized("Invalid client credentials".to_string()));
    }

    Ok(client)
}