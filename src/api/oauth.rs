use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
    routing::post,
    Json, Router,
};
use axum_extra::headers::{authorization::Bearer, Authorization, HeaderMapExt};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::{info, warn};

use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    models::{AppState, AuthResponse, Db, LoginRequest, NewUser, User},
};

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub email_verified: Option<bool>,
}

/// Регистрация нового пользователя
#[utoipa::path(
    post,
    path = "/oauth/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User created successfully", body = AuthResponse),
        (status = 409, description = "Email already exists"),
        (status = 400, description = "Validation error")
    )
)]
#[axum::debug_handler]
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterRequest>,
) -> AppResult<Json<AuthResponse>> {
    info!("Register attempt for email: {}", payload.email);

    // Проверяем, существует ли пользователь с таким email
    if User::find_by_email(&state.pool, &payload.email)
        .await
        .map_err(|e| AppError::Other(e.into()))?
        .is_some()
    {
        return Err(AppError::EmailAlreadyExists);
    }

    // Создаем пользователя
    let new_user = NewUser {
        email: payload.email.clone(),
        password: payload.password.clone(),
        email_verified: payload.email_verified,
    };
    let user = User::create(&state.pool, &new_user)
        .await
        .map_err(|e| AppError::Other(e.into()))?;

    // Генерируем JWT токен
    let token = crate::models::create_jwt(user.id.clone(), &state.jwt_secret)
        .map_err(|e| AppError::Other(e.into()))?;

    Ok(Json(AuthResponse { token, user }))
}

/// Вход пользователя
#[utoipa::path(
    post,
    path = "/oauth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials"),
        (status = 404, description = "User not found")
    )
)]
#[axum::debug_handler]
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    info!("Login attempt for email: {}", payload.email);

    // Находим пользователя по email
    let user = User::find_by_email(&state.pool, &payload.email)
        .await
        .map_err(|e| AppError::Other(e.into()))?
        .ok_or(AppError::InvalidCredentials)?;

    // Проверяем пароль
    if !user
        .verify_password(&payload.password)
        .map_err(|e| AppError::Other(e.into()))?
    {
        return Err(AppError::InvalidCredentials);
    }

    // Генерируем JWT токен
    let token = crate::models::create_jwt(user.id.clone(), &state.jwt_secret)
        .map_err(|e| AppError::Other(e.into()))?;

    Ok(Json(AuthResponse { token, user }))
}


/// Middleware для проверки аутентификации
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> AppResult<Response> {
    // Извлекаем токен из заголовка Authorization
    let token = req
        .headers()
        .typed_get::<Authorization<Bearer>>()
        .ok_or_else(|| {
            warn!("Missing Authorization header");
            AppError::Unauthorized
        })?
        .token()
        .to_string();

    // Валидируем JWT токен
    let claims = crate::models::validate_jwt(&token, &state.jwt_secret)
        .map_err(|e| {
            warn!("JWT validation failed: {}", e);
            AppError::Unauthorized
        })?;

    // Добавляем user_id в extensions запроса для использования в handlers
    req.extensions_mut().insert(claims.sub);

    Ok(next.run(req).await)
}

/// Простой middleware для проверки аутентификации без состояния
/// Использует JWT секрет из extensions запроса
pub async fn simple_auth_middleware(
    mut req: Request,
    next: Next,
) -> AppResult<Response> {
    // Извлекаем токен из заголовка Authorization
    let token = req
        .headers()
        .typed_get::<Authorization<Bearer>>()
        .ok_or_else(|| {
            warn!("Missing Authorization header");
            AppError::Unauthorized
        })?
        .token()
        .to_string();

    // Получаем JWT секрет из extensions
    let jwt_secret = req
        .extensions()
        .get::<String>()
        .ok_or_else(|| {
            warn!("JWT secret not found in extensions");
            AppError::Unauthorized
        })?;

    // Валидируем JWT токен
    let claims = crate::models::validate_jwt(&token, jwt_secret)
        .map_err(|e| {
            warn!("JWT validation failed: {}", e);
            AppError::Unauthorized
        })?;

    // Добавляем user_id в extensions запроса для использования в handlers
    req.extensions_mut().insert(claims.sub);

    Ok(next.run(req).await)
}

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

/// Extractor для получения user_id из запроса
#[derive(Debug, Clone)]
pub struct AuthenticatedUser(pub String);

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let user_id = parts
            .extensions
            .get::<String>()
            .ok_or_else(|| AppError::Unauthorized)?;

        Ok(AuthenticatedUser(user_id.clone()))
    }
}

use crate::models::{
    create_access_token, create_refresh_token, get_allowed_scopes, validate_access_token,
    validate_refresh_token, validate_scope, OAuthRevokeRequest, OAuthTokenRequest,
    OAuthTokenResponse, UserInfoResponse,
};

/// OAuth 2.0 Token endpoint
/// Поддерживает grant types: password и refresh_token
#[utoipa::path(
    post,
    path = "/oauth/token",
    request_body = OAuthTokenRequest,
    responses(
        (status = 200, description = "Token issued successfully", body = OAuthTokenResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Invalid credentials"),
        (status = 403, description = "Invalid scope")
    )
)]
#[axum::debug_handler]
pub async fn oauth_token(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OAuthTokenRequest>,
) -> AppResult<Json<OAuthTokenResponse>> {
    info!("OAuth token request with grant_type: {}", payload.grant_type);

    let allowed_scopes = get_allowed_scopes();
    
    // Валидация scope
    if !validate_scope(payload.scope.as_deref(), &allowed_scopes) {
        return Err(AppError::InvalidScope);
    }

    match payload.grant_type.as_str() {
        "password" => {
            // Resource Owner Password Credentials grant
            let username = payload.username.ok_or(AppError::InvalidRequest)?;
            let password = payload.password.ok_or(AppError::InvalidRequest)?;

            // Находим пользователя по email
            let user = User::find_by_email(&state.pool, &username)
                .await
                .map_err(|e| AppError::Other(e.into()))?
                .ok_or(AppError::InvalidCredentials)?;

            // Проверяем пароль
            if !user
                .verify_password(&password)
                .map_err(|e| AppError::Other(e.into()))?
            {
                return Err(AppError::InvalidCredentials);
            }

            // Генерируем access и refresh токены
            let access_token = create_access_token(user.id.clone(), &state.jwt_secret)
                .map_err(|e| AppError::Other(e.into()))?;
            
            let refresh_token = create_refresh_token(user.id.clone(), &state.jwt_secret)
                .map_err(|e| AppError::Other(e.into()))?;

            Ok(Json(OAuthTokenResponse {
                access_token,
                token_type: "Bearer".to_string(),
                expires_in: 3600, // 1 час
                refresh_token: Some(refresh_token),
                scope: payload.scope,
            }))
        }
        "refresh_token" => {
            // Refresh Token grant
            let refresh_token = payload.refresh_token.ok_or(AppError::InvalidRequest)?;

            // Валидируем refresh token
            let claims = validate_refresh_token(&refresh_token, &state.jwt_secret)
                .map_err(|_| AppError::InvalidToken)?;

            // Генерируем новый access token
            let access_token = create_access_token(claims.sub.clone(), &state.jwt_secret)
                .map_err(|e| AppError::Other(e.into()))?;

            Ok(Json(OAuthTokenResponse {
                access_token,
                token_type: "Bearer".to_string(),
                expires_in: 3600, // 1 час
                refresh_token: Some(refresh_token), // Возвращаем тот же refresh token
                scope: payload.scope,
            }))
        }
        _ => Err(AppError::InvalidGrantType),
    }
}

/// OAuth 2.0 Revoke endpoint
/// Отзывает access или refresh токены
#[utoipa::path(
    post,
    path = "/oauth/revoke",
    request_body = OAuthRevokeRequest,
    responses(
        (status = 200, description = "Token revoked successfully"),
        (status = 400, description = "Invalid request")
    )
)]
#[axum::debug_handler]
pub async fn oauth_revoke(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OAuthRevokeRequest>,
) -> AppResult<()> {
    info!("OAuth revoke request");

    // В реальной реализации здесь должна быть проверка client credentials
    // и добавление токена в blacklist
    
    // Для простоты просто валидируем токен, чтобы убедиться что он действительный
    match payload.token_type_hint.as_deref() {
        Some("access_token") => {
            validate_access_token(&payload.token, &state.jwt_secret)
                .map_err(|_| AppError::InvalidToken)?;
        }
        Some("refresh_token") => {
            validate_refresh_token(&payload.token, &state.jwt_secret)
                .map_err(|_| AppError::InvalidToken)?;
        }
        _ => {
            // Пытаемся определить тип токена автоматически
            if validate_access_token(&payload.token, &state.jwt_secret).is_ok() {
                // Это access token
            } else if validate_refresh_token(&payload.token, &state.jwt_secret).is_ok() {
                // Это refresh token
            } else {
                return Err(AppError::InvalidToken);
            }
        }
    }

    // В реальном приложении здесь нужно добавить токен в blacklist базу данных
    // и обеспечить его отзыв при последующих проверках

    Ok(())
}

/// UserInfo endpoint
/// Возвращает информацию о пользователе на основе access token
#[utoipa::path(
    get,
    path = "/userinfo",
    responses(
        (status = 200, description = "User info retrieved successfully", body = UserInfoResponse),
        (status = 401, description = "Invalid token")
    ),
    security(
        ("oauth2" = [])
    )
)]
#[axum::debug_handler]
pub async fn userinfo(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user_id): AuthenticatedUser,
) -> AppResult<Json<UserInfoResponse>> {
    info!("UserInfo request for user: {}", user_id);

    // Находим пользователя по ID
    let user = User::find_by_id(&state.pool, &user_id)
        .await
        .map_err(|e| AppError::Other(e.into()))?
        .ok_or(AppError::UserNotFound)?;

    Ok(Json(UserInfoResponse {
        sub: user.id,
        email: user.email,
        email_verified: user.email_verified,
        name: None, // Эти поля можно добавить в будущем
        given_name: None,
        family_name: None,
        picture: None,
        locale: None,
        updated_at: Some(user.updated_at),
    }))
}

// Вспомогательная функция для поиска пользователя по ID
impl User {
    pub async fn find_by_id(pool: &Db, id: &str) -> Result<Option<Self>, anyhow::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, email, password_hash, email_verified, verification_token,
                   reset_token, reset_token_expires, created_at, updated_at
            FROM users
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = row {
            let user = User {
                id: row.get("id"),
                email: row.get("email"),
                password_hash: row.get("password_hash"),
                email_verified: row.get("email_verified"),
                verification_token: row.get("verification_token"),
                reset_token: row.get("reset_token"),
                reset_token_expires: row.get("reset_token_expires"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            };
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/token", post(oauth_token))
        .route("/revoke", post(oauth_revoke))
        .route("/userinfo", axum::routing::get(userinfo))
}
