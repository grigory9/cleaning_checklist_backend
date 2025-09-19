use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row, SqlitePool};
use utoipa::ToSchema;

pub type Db = SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: Db,
    pub jwt_secret: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Custom,
}

impl Frequency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Frequency::Daily => "daily",
            Frequency::Weekly => "weekly",
            Frequency::Monthly => "monthly",
            Frequency::Custom => "custom",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "daily" => Some(Frequency::Daily),
            "weekly" => Some(Frequency::Weekly),
            "monthly" => Some(Frequency::Monthly),
            "custom" => Some(Frequency::Custom),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow, Clone)]
pub struct Room {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct RoomView {
    #[schema(example = "b0f7462c-6ca0-4a2a-9b77-1a64f1d76b2c")]
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub zones_total: Option<i64>,
    pub zones_cleaned_count: Option<i64>,
    pub last_cleaned_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NewRoom { pub name: String, pub icon: Option<String> }

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateRoom { pub name: Option<String>, pub icon: Option<String> }

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow, Clone)]
pub struct Zone {
    pub id: String,
    pub room_id: String,
    pub name: String,
    pub icon: Option<String>,
    pub frequency: String,
    pub custom_interval_days: Option<i64>,
    pub last_cleaned_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct ZoneView {
    pub id: String,
    pub room_id: String,
    pub name: String,
    pub icon: Option<String>,
    pub frequency: String,
    pub custom_interval_days: Option<i64>,
    pub last_cleaned_at: Option<DateTime<Utc>>,
    pub next_due_at: Option<DateTime<Utc>>,
    pub is_due: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NewZone {
    pub name: String,
    pub icon: Option<String>,
    pub frequency: Frequency,
    pub custom_interval_days: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateZone {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub frequency: Option<Frequency>,
    pub custom_interval_days: Option<u16>,
}

pub fn compute_next_due(last: Option<DateTime<Utc>>, freq: &str, custom: Option<i64>) -> Option<DateTime<Utc>> {
    let last = last?;
    match Frequency::from_str(freq) {
        Some(Frequency::Daily) => Some(last + chrono::Duration::days(1)),
        Some(Frequency::Weekly) => Some(last + chrono::Duration::weeks(1)),
        Some(Frequency::Monthly) => Some(last + chrono::Duration::days(30)), // упрощённо
        Some(Frequency::Custom) => Some(last + chrono::Duration::days(custom.unwrap_or(1))),
        None => None,
    }
}

pub fn compute_is_due(next_due: Option<DateTime<Utc>>) -> bool {
    match next_due {
        Some(dt) => chrono::Utc::now() >= dt,
        None => true, // если уборки не было — просрочено
    }
}

// Аутентификация и пользователи
#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub email_verified: bool,
    pub verification_token: Option<String>,
    pub reset_token: Option<String>,
    pub reset_token_expires: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NewUser {
    pub email: String,
    pub password: String,
    pub email_verified: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

// Функции для работы с пользователями
impl User {
    pub async fn create(pool: &Db, new_user: &NewUser) -> Result<Self, anyhow::Error> {
        let password_hash = hash_password(&new_user.password)?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let email_verified = new_user.email_verified.unwrap_or(false);

        sqlx::query!(
            r#"
            INSERT INTO users (id, email, password_hash, email_verified, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            id,
            new_user.email,
            password_hash,
            email_verified,
            now,
            now
        )
        .execute(pool)
        .await?;

        Ok(Self {
            id,
            email: new_user.email.clone(),
            password_hash,
            email_verified,
            verification_token: None,
            reset_token: None,
            reset_token_expires: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn find_by_email(pool: &Db, email: &str) -> Result<Option<Self>, anyhow::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, email, password_hash, email_verified, verification_token,
                   reset_token, reset_token_expires, created_at, updated_at
            FROM users
            WHERE email = ?
            "#,
        )
        .bind(email)
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

    pub fn verify_password(&self, password: &str) -> Result<bool, anyhow::Error> {
        verify_password(password, &self.password_hash)
    }
}

// Хеширование паролей
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

pub fn hash_password(password: &str) -> Result<String, anyhow::Error> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(password_hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, anyhow::Error> {
    let parsed_hash = PasswordHash::new(hash)?;
    let argon2 = Argon2::default();
    Ok(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok())
}

// JWT аутентификация
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user id
    pub exp: usize,  // expiration time
    pub iat: usize,  // issued at
}

impl Claims {
    pub fn new(user_id: String, expiration_days: i64) -> Self {
        let now = chrono::Utc::now();
        let exp = now + chrono::Duration::days(expiration_days);
        
        Self {
            sub: user_id,
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
        }
    }
}

pub fn create_jwt(user_id: String, secret: &str) -> Result<String, anyhow::Error> {
    let claims = Claims::new(user_id, 30); // 30 дней expiration
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    
    Ok(token)
}

pub fn validate_jwt(token: &str, secret: &str) -> Result<Claims, anyhow::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    
    Ok(token_data.claims)
}

// OAuth 2.0 структуры и функции
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OAuthTokenRequest {
    pub grant_type: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: usize,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OAuthRevokeRequest {
    pub token: String,
    pub token_type_hint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserInfoResponse {
    pub sub: String,
    pub email: String,
    pub email_verified: bool,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub picture: Option<String>,
    pub locale: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

// Функции для работы с OAuth 2.0 токенами
pub fn create_access_token(user_id: String, secret: &str) -> Result<String, anyhow::Error> {
    let claims = Claims::new(user_id, 1); // 1 час expiration для access token
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    
    Ok(token)
}

pub fn create_refresh_token(user_id: String, secret: &str) -> Result<String, anyhow::Error> {
    let claims = Claims::new(user_id, 30); // 30 дней expiration для refresh token
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    
    Ok(token)
}

pub fn validate_access_token(token: &str, secret: &str) -> Result<Claims, anyhow::Error> {
    let mut validation = Validation::default();
    validation.validate_exp = true;
    
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    
    Ok(token_data.claims)
}

pub fn validate_refresh_token(token: &str, secret: &str) -> Result<Claims, anyhow::Error> {
    let mut validation = Validation::default();
    validation.validate_exp = true;
    
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    
    Ok(token_data.claims)
}

// Валидация scope
pub fn validate_scope(requested_scope: Option<&str>, allowed_scopes: &[&str]) -> bool {
    if let Some(scope) = requested_scope {
        let requested_scopes: Vec<&str> = scope.split_whitespace().collect();
        requested_scopes.iter().all(|s| allowed_scopes.contains(s))
    } else {
        true // Если scope не указан, разрешаем доступ к базовым scope
    }
}

// Получение разрешенных scope для пользователя
pub fn get_allowed_scopes() -> Vec<&'static str> {
    vec![
        "openid",
        "profile",
        "email",
        "offline_access",
    ]
}
