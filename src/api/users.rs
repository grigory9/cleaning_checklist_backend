use axum::{
    extract::State,
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    auth::password::{hash_password, verify_password},
    error::{AppError, AppResult},
    models::{AppState, LoginUser, RegisterUser, User, UserView},
};

#[utoipa::path(
    post,
    path = "/register",
    request_body = RegisterUser,
    responses((status = 201, description = "User registered successfully", body = UserView)),
    tag = "auth"
)]
pub async fn register(
    State(state): State<std::sync::Arc<AppState>>,
    Json(payload): Json<RegisterUser>,
) -> AppResult<Json<UserView>> {
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

    Ok(Json(user.into()))
}

#[utoipa::path(
    post,
    path = "/login",
    request_body = LoginUser,
    responses((status = 200, description = "Login successful", body = UserView)),
    tag = "auth"
)]
pub async fn login(
    State(state): State<std::sync::Arc<AppState>>,
    Json(payload): Json<LoginUser>,
) -> AppResult<Json<UserView>> {
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

    Ok(Json(user.into()))
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