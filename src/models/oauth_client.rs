use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow, Clone)]
pub struct OAuthClient {
    pub client_id: String,
    #[serde(skip_serializing)]
    pub client_secret_hash: String,
    pub name: String,
    pub redirect_uris: String,  // JSON array as string
    pub grant_types: String,    // JSON array as string
    pub scopes: String,         // JSON array as string
    pub created_at: DateTime<Utc>,
    pub is_public: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OAuthClientView {
    pub client_id: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub scopes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub is_public: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateOAuthClient {
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub scopes: Vec<String>,
    pub is_public: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OAuthClientCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub scopes: Vec<String>,
    pub is_public: bool,
}

impl OAuthClient {
    pub fn to_view(&self) -> Result<OAuthClientView, serde_json::Error> {
        Ok(OAuthClientView {
            client_id: self.client_id.clone(),
            name: self.name.clone(),
            redirect_uris: serde_json::from_str(&self.redirect_uris)?,
            grant_types: serde_json::from_str(&self.grant_types)?,
            scopes: serde_json::from_str(&self.scopes)?,
            created_at: self.created_at,
            is_public: self.is_public,
        })
    }

    pub fn get_redirect_uris(&self) -> Result<Vec<String>, serde_json::Error> {
        serde_json::from_str(&self.redirect_uris)
    }

    pub fn get_grant_types(&self) -> Result<Vec<String>, serde_json::Error> {
        serde_json::from_str(&self.grant_types)
    }

    pub fn get_scopes(&self) -> Result<Vec<String>, serde_json::Error> {
        serde_json::from_str(&self.scopes)
    }
}