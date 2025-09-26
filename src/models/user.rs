use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub email_verified: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct UserView {
    pub id: String,
    pub email: String,
    pub username: String,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub email_verified: bool,
}

impl From<User> for UserView {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            username: user.username,
            name: user.name,
            created_at: user.created_at,
            updated_at: user.updated_at,
            email_verified: user.email_verified,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterUser {
    #[schema(example = "user@example.com")]
    pub email: String,
    #[schema(example = "username")]
    pub username: String,
    #[schema(example = "securepassword123")]
    pub password: String,
    #[schema(example = "John Doe")]
    pub name: Option<String>,
    #[schema(example = "2ab18a2b-bb0a-4485-ac3a-7ac6d93ab2fa")]
    pub client_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginUser {
    #[schema(example = "user@example.com")]
    pub email: String,
    #[schema(example = "securepassword123")]
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateUser {
    pub email: Option<String>,
    pub username: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChangePassword {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserView,
}