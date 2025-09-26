use axum::{
    extract::State,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    auth::{
        password::hash_password,
        tokens::generate_client_secret,
    },
    error::{AppError, AppResult},
    models::{AppState, OAuthClient, OAuthClientCredentials},
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateClientRequest {
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub scopes: Vec<String>,
    pub is_public: Option<bool>,
}

// POST /admin/clients - Create OAuth client
#[utoipa::path(
    post,
    path = "/admin/clients",
    request_body = CreateClientRequest,
    responses((status = 200, description = "OAuth client created", body = OAuthClientCredentials)),
    tag = "admin"
)]
pub async fn create_client(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateClientRequest>,
) -> AppResult<Json<OAuthClientCredentials>> {
    let client_id = Uuid::new_v4().to_string();
    let client_secret = generate_client_secret();
    let is_public = req.is_public.unwrap_or(false);

    // Hash the client secret (only if not public)
    let client_secret_hash = if is_public {
        String::new() // No secret for public clients
    } else {
        hash_password(&client_secret)
            .map_err(|e| AppError::Other(anyhow::anyhow!("Failed to hash client secret: {}", e)))?
    };

    // Serialize arrays to JSON
    let redirect_uris_json = serde_json::to_string(&req.redirect_uris)
        .map_err(|e| AppError::BadRequest(format!("Invalid redirect URIs: {}", e)))?;

    let grant_types_json = serde_json::to_string(&req.grant_types)
        .map_err(|e| AppError::BadRequest(format!("Invalid grant types: {}", e)))?;

    let scopes_json = serde_json::to_string(&req.scopes)
        .map_err(|e| AppError::BadRequest(format!("Invalid scopes: {}", e)))?;

    // Insert into database
    sqlx::query(
        r#"INSERT INTO oauth_clients
           (client_id, client_secret_hash, name, redirect_uris, grant_types, scopes, created_at, is_public)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
    )
    .bind(&client_id)
    .bind(&client_secret_hash)
    .bind(&req.name)
    .bind(&redirect_uris_json)
    .bind(&grant_types_json)
    .bind(&scopes_json)
    .bind(&Utc::now())
    .bind(is_public)
    .execute(&state.pool)
    .await?;

    Ok(Json(OAuthClientCredentials {
        client_id,
        client_secret: if is_public { String::new() } else { client_secret },
        name: req.name,
        redirect_uris: req.redirect_uris,
        grant_types: req.grant_types,
        scopes: req.scopes,
        is_public,
    }))
}

// GET /admin/clients - List OAuth clients
#[utoipa::path(
    get,
    path = "/admin/clients",
    responses((status = 200, description = "List of OAuth clients", body = [OAuthClient])),
    tag = "admin"
)]
pub async fn list_clients(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<OAuthClient>>> {
    let clients = sqlx::query_as::<_, OAuthClient>(
        "SELECT * FROM oauth_clients ORDER BY created_at DESC"
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(clients))
}