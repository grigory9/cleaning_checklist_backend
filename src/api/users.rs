use axum::{
    extract::State,
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    auth::{
        password::{hash_password, verify_password},
        scopes::{Scope, ScopeSet},
        tokens::{hash_token, TokenGenerator},
    },
    error::{AppError, AppResult},
    models::{AppState, AuthResponse, LoginUser, RefreshTokenRequest, RegisterUser, User, UserView},
};
use chrono::Duration;

#[utoipa::path(
    post,
    path = "/register",
    request_body = RegisterUser,
    responses((status = 201, description = "User registered successfully", body = AuthResponse)),
    tag = "auth"
)]
pub async fn register(
    State(state): State<std::sync::Arc<AppState>>,
    Json(payload): Json<RegisterUser>,
) -> AppResult<Json<AuthResponse>> {
    // Validate input
    if payload.email.trim().is_empty() {
        return Err(AppError::BadRequest("Email is required".to_string()));
    }

    if payload.username.trim().is_empty() {
        return Err(AppError::BadRequest("Username is required".to_string()));
    }

    if payload.password.len() < 8 {
        return Err(AppError::BadRequest("Password must be at least 8 characters long".to_string()));
    }

    // Check if user already exists
    let existing_user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = ?1 OR username = ?2"
    )
    .bind(&payload.email)
    .bind(&payload.username)
    .fetch_optional(&state.pool)
    .await?;

    if existing_user.is_some() {
        return Err(AppError::Conflict("User with this email or username already exists".to_string()));
    }

    // Hash password
    let password_hash = hash_password(&payload.password)
        .map_err(|e| AppError::Other(anyhow::anyhow!("Password hashing failed: {}", e)))?;

    // Create user
    let user_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (id, email, username, password_hash, name, created_at, updated_at, email_verified)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
           RETURNING *"#,
    )
    .bind(&user_id)
    .bind(&payload.email)
    .bind(&payload.username)
    .bind(&password_hash)
    .bind(&payload.name)
    .bind(&now)
    .bind(&now)
    .bind(false)
    .fetch_one(&state.pool)
    .await?;

    // Generate JWT token for the new user
    let scopes = ScopeSet::from_vec(vec![
        Scope::RoomsRead,
        Scope::RoomsWrite,
        Scope::ZonesRead,
        Scope::ZonesWrite,
        Scope::StatsRead,
    ]);

    let token_generator = TokenGenerator::new()
        .map_err(|e| AppError::Other(anyhow::anyhow!("Token generator initialization failed: {}", e)))?;

    // Use client_id from request or fall back to default
    let client_id = payload.client_id.as_deref().unwrap_or("2ab18a2b-bb0a-4485-ac3a-7ac6d93ab2fa");

    let (access_token, access_jti) = token_generator
        .generate_access_token(Some(&user.id), client_id, &scopes, 60 * 24) // 24 hour token
        .map_err(|e| AppError::Other(anyhow::anyhow!("Access token generation failed: {}", e)))?;

    let (refresh_token, refresh_jti) = token_generator
        .generate_refresh_token(Some(&user.id), client_id, &scopes, 60 * 24 * 30) // 30 day token
        .map_err(|e| AppError::Other(anyhow::anyhow!("Refresh token generation failed: {}", e)))?;

    let access_expires_at = Utc::now() + Duration::minutes(60 * 24); // 24 hours
    let refresh_expires_at = Utc::now() + Duration::minutes(60 * 24 * 30); // 30 days

    // Store access token in database
    sqlx::query(
        r#"INSERT INTO access_tokens
           (token_hash, client_id, user_id, scopes, expires_at, created_at, revoked)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
    )
    .bind(hash_token(&access_jti))
    .bind(client_id)
    .bind(&user.id)
    .bind(scopes.to_json_array().to_string())
    .bind(&access_expires_at)
    .bind(&Utc::now())
    .bind(false)
    .execute(&state.pool)
    .await?;

    // Store refresh token in database
    sqlx::query(
        r#"INSERT INTO refresh_tokens
           (token_hash, client_id, user_id, scopes, expires_at, created_at, revoked)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
    )
    .bind(hash_token(&refresh_jti))
    .bind(client_id)
    .bind(&user.id)
    .bind(scopes.to_json_array().to_string())
    .bind(&refresh_expires_at)
    .bind(&Utc::now())
    .bind(false)
    .execute(&state.pool)
    .await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: 60 * 60 * 24, // 24 hours in seconds
        user: user.into(),
    }))
}

#[utoipa::path(
    post,
    path = "/login",
    request_body = LoginUser,
    responses((status = 200, description = "Login successful", body = AuthResponse)),
    tag = "auth"
)]
pub async fn login(
    State(state): State<std::sync::Arc<AppState>>,
    Json(payload): Json<LoginUser>,
) -> AppResult<Json<AuthResponse>> {
    // Find user by email
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = ?1"
    )
    .bind(&payload.email)
    .fetch_optional(&state.pool)
    .await?;

    let user = user.ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    // Verify password
    let is_valid = verify_password(&payload.password, &user.password_hash)
        .map_err(|e| AppError::Other(anyhow::anyhow!("Password verification failed: {}", e)))?;

    if !is_valid {
        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    // Generate JWT token for the authenticated user
    let scopes = ScopeSet::from_vec(vec![
        Scope::RoomsRead,
        Scope::RoomsWrite,
        Scope::ZonesRead,
        Scope::ZonesWrite,
        Scope::StatsRead,
    ]);

    let token_generator = TokenGenerator::new()
        .map_err(|e| AppError::Other(anyhow::anyhow!("Token generator initialization failed: {}", e)))?;

    // Use 'ios' client_id since it exists in the database
    let client_id = "ios";

    let (access_token, access_jti) = token_generator
        .generate_access_token(Some(&user.id), client_id, &scopes, 60 * 24) // 24 hour token
        .map_err(|e| AppError::Other(anyhow::anyhow!("Access token generation failed: {}", e)))?;

    let (refresh_token, refresh_jti) = token_generator
        .generate_refresh_token(Some(&user.id), client_id, &scopes, 60 * 24 * 30) // 30 day token
        .map_err(|e| AppError::Other(anyhow::anyhow!("Refresh token generation failed: {}", e)))?;

    let access_expires_at = Utc::now() + Duration::minutes(60 * 24); // 24 hours
    let refresh_expires_at = Utc::now() + Duration::minutes(60 * 24 * 30); // 30 days

    // Store access token in database
    sqlx::query(
        r#"INSERT INTO access_tokens
           (token_hash, client_id, user_id, scopes, expires_at, created_at, revoked)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
    )
    .bind(hash_token(&access_jti))
    .bind(client_id)
    .bind(&user.id)
    .bind(scopes.to_json_array().to_string())
    .bind(&access_expires_at)
    .bind(&Utc::now())
    .bind(false)
    .execute(&state.pool)
    .await?;

    // Store refresh token in database
    sqlx::query(
        r#"INSERT INTO refresh_tokens
           (token_hash, client_id, user_id, scopes, expires_at, created_at, revoked)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
    )
    .bind(hash_token(&refresh_jti))
    .bind(client_id)
    .bind(&user.id)
    .bind(scopes.to_json_array().to_string())
    .bind(&refresh_expires_at)
    .bind(&Utc::now())
    .bind(false)
    .execute(&state.pool)
    .await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: 60 * 60 * 24, // 24 hours in seconds
        user: user.into(),
    }))
}

#[utoipa::path(
    get,
    path = "/me",
    responses((status = 200, description = "Current user info", body = UserView)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub async fn me(
    auth_user: crate::auth::middleware::AuthUser,
) -> AppResult<Json<UserView>> {
    Ok(Json(auth_user.user.into()))
}

#[utoipa::path(
    post,
    path = "/refresh",
    request_body = RefreshTokenRequest,
    responses((status = 200, description = "Token refreshed successfully", body = AuthResponse)),
    tag = "auth"
)]
pub async fn refresh_token(
    State(state): State<std::sync::Arc<AppState>>,
    Json(payload): Json<RefreshTokenRequest>,
) -> AppResult<Json<AuthResponse>> {
    let token_generator = TokenGenerator::new()
        .map_err(|e| AppError::Other(anyhow::anyhow!("Token generator initialization failed: {}", e)))?;

    // Validate the refresh token
    let refresh_claims = token_generator
        .validate_refresh_token(&payload.refresh_token)
        .map_err(|_| AppError::Unauthorized("Invalid refresh token".to_string()))?;

    // Check if refresh token exists in database and is not revoked
    let refresh_token_hash = hash_token(&refresh_claims.jti);
    let refresh_token_record = sqlx::query!(
        "SELECT user_id, client_id, scopes, revoked FROM refresh_tokens WHERE token_hash = ?1 AND expires_at > datetime('now')",
        refresh_token_hash
    )
    .fetch_optional(&state.pool)
    .await?;

    let refresh_token_record = refresh_token_record
        .ok_or_else(|| AppError::Unauthorized("Refresh token not found or expired".to_string()))?;

    if refresh_token_record.revoked.unwrap_or(false) {
        return Err(AppError::Unauthorized("Refresh token has been revoked".to_string()));
    }

    // Get user information
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = ?1"
    )
    .bind(&refresh_token_record.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

    // Parse scopes from stored token (stored as JSON array)
    let scopes_json: serde_json::Value = serde_json::from_str(&refresh_token_record.scopes)
        .map_err(|e| AppError::Other(anyhow::anyhow!("Invalid scopes JSON: {}", e)))?;
    let scopes = ScopeSet::from_json_array(&scopes_json)
        .map_err(|e| AppError::Other(anyhow::anyhow!("Invalid scopes: {}", e)))?;

    // Generate new access token
    let (new_access_token, access_jti) = token_generator
        .generate_access_token(Some(&user.id), &refresh_token_record.client_id, &scopes, 60 * 24) // 24 hour token
        .map_err(|e| AppError::Other(anyhow::anyhow!("Access token generation failed: {}", e)))?;

    // Generate new refresh token (rotate refresh tokens for security)
    let (new_refresh_token, refresh_jti) = token_generator
        .generate_refresh_token(Some(&user.id), &refresh_token_record.client_id, &scopes, 60 * 24 * 30) // 30 day token
        .map_err(|e| AppError::Other(anyhow::anyhow!("Refresh token generation failed: {}", e)))?;

    let access_expires_at = Utc::now() + Duration::minutes(60 * 24); // 24 hours
    let refresh_expires_at = Utc::now() + Duration::minutes(60 * 24 * 30); // 30 days

    // Store new access token in database
    sqlx::query(
        r#"INSERT INTO access_tokens
           (token_hash, client_id, user_id, scopes, expires_at, created_at, revoked)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
    )
    .bind(hash_token(&access_jti))
    .bind(&refresh_token_record.client_id)
    .bind(&user.id)
    .bind(&refresh_token_record.scopes)
    .bind(&access_expires_at)
    .bind(&Utc::now())
    .bind(false)
    .execute(&state.pool)
    .await?;

    // Revoke old refresh token and store new one
    sqlx::query("UPDATE refresh_tokens SET revoked = TRUE WHERE token_hash = ?1")
        .bind(&refresh_token_hash)
        .execute(&state.pool)
        .await?;

    // Store new refresh token in database
    sqlx::query(
        r#"INSERT INTO refresh_tokens
           (token_hash, client_id, user_id, scopes, expires_at, created_at, revoked)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
    )
    .bind(hash_token(&refresh_jti))
    .bind(&refresh_token_record.client_id)
    .bind(&user.id)
    .bind(&refresh_token_record.scopes)
    .bind(&refresh_expires_at)
    .bind(&Utc::now())
    .bind(false)
    .execute(&state.pool)
    .await?;

    Ok(Json(AuthResponse {
        access_token: new_access_token,
        refresh_token: new_refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: 60 * 60 * 24, // 24 hours in seconds
        user: user.into(),
    }))
}