use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::IntoParams;

use crate::{
    auth::tokens::generate_authorization_code,
    error::{AppError, AppResult},
    models::{AppState, OAuthClient},
};

#[derive(Debug, Deserialize, IntoParams)]
pub struct AuthorizeParams {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConsentForm {
    pub user_id: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub approved: String, // "true" or "false"
}

// GET /oauth/authorize - Authorization request (shows consent page)
#[utoipa::path(
    get,
    path = "/oauth/authorize",
    params(AuthorizeParams),
    responses(
        (status = 200, description = "Authorization form", content_type = "text/html"),
        (status = 302, description = "Redirect with error"),
    )
)]
pub async fn authorize_get(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuthorizeParams>,
) -> AppResult<Response> {
    // Validate response_type
    if params.response_type != "code" {
        return error_redirect(
            &params.redirect_uri,
            "unsupported_response_type",
            "Only 'code' response type is supported",
            params.state.as_deref(),
        );
    }

    // Validate client
    let client = match get_oauth_client(&state, &params.client_id).await? {
        Some(client) => client,
        None => {
            return error_redirect(
                &params.redirect_uri,
                "invalid_client",
                "Client not found",
                params.state.as_deref(),
            );
        }
    };

    // Validate redirect_uri
    let allowed_uris: Vec<String> = serde_json::from_str(&client.redirect_uris)
        .map_err(|_| AppError::BadRequest("Invalid client redirect URIs".to_string()))?;

    if !allowed_uris.contains(&params.redirect_uri) {
        return error_redirect(
            &params.redirect_uri,
            "invalid_request",
            "Invalid redirect_uri",
            params.state.as_deref(),
        );
    }

    // Validate PKCE if present
    if params.code_challenge.is_some() {
        let method = params.code_challenge_method.as_deref().unwrap_or("plain");
        if method != "S256" && method != "plain" {
            return error_redirect(
                &params.redirect_uri,
                "invalid_request",
                "Invalid code_challenge_method",
                params.state.as_deref(),
            );
        }
    }

    // For demo purposes, we'll show a simple consent form
    // In a real implementation, this would check if the user is authenticated
    // and show a proper consent page
    let consent_html = generate_consent_page(&params, &client);
    Ok(Html(consent_html).into_response())
}

// POST /oauth/authorize - Handle consent form submission
#[utoipa::path(
    post,
    path = "/oauth/authorize",
    request_body = ConsentForm,
    responses(
        (status = 302, description = "Redirect with authorization code or error"),
    )
)]
pub async fn authorize_post(
    State(state): State<Arc<AppState>>,
    Form(form): Form<ConsentForm>,
) -> AppResult<Response> {
    // Check if user approved the request
    if form.approved != "true" {
        return error_redirect(
            &form.redirect_uri,
            "access_denied",
            "User denied the authorization request",
            form.state.as_deref(),
        );
    }

    // Validate client again
    let client = match get_oauth_client(&state, &form.client_id).await? {
        Some(client) => client,
        None => {
            return error_redirect(
                &form.redirect_uri,
                "invalid_client",
                "Client not found",
                form.state.as_deref(),
            );
        }
    };

    // Generate authorization code
    let code = generate_authorization_code();
    let expires_at = Utc::now() + Duration::minutes(10); // 10 minute expiry

    // Store authorization code in database
    sqlx::query(
        r#"INSERT INTO authorization_codes
           (code, client_id, user_id, redirect_uri, scopes, expires_at, code_challenge, code_challenge_method, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
    )
    .bind(&code)
    .bind(&form.client_id)
    .bind(&form.user_id)
    .bind(&form.redirect_uri)
    .bind(&form.scope)
    .bind(&expires_at)
    .bind(&form.code_challenge)
    .bind(&form.code_challenge_method)
    .bind(&Utc::now())
    .execute(&state.pool)
    .await?;

    // Redirect back to client with authorization code
    let mut redirect_url = url::Url::parse(&form.redirect_uri)
        .map_err(|_| AppError::BadRequest("Invalid redirect_uri".to_string()))?;

    redirect_url.query_pairs_mut().append_pair("code", &code);
    if let Some(ref state_param) = form.state {
        redirect_url.query_pairs_mut().append_pair("state", state_param);
    }

    Ok(Redirect::to(&redirect_url.to_string()).into_response())
}

async fn get_oauth_client(
    state: &Arc<AppState>,
    client_id: &str,
) -> AppResult<Option<OAuthClient>> {
    let client = sqlx::query_as::<_, OAuthClient>(
        "SELECT * FROM oauth_clients WHERE client_id = ?1"
    )
    .bind(client_id)
    .fetch_optional(&state.pool)
    .await?;

    Ok(client)
}

fn error_redirect(
    redirect_uri: &str,
    error: &str,
    error_description: &str,
    state: Option<&str>,
) -> AppResult<Response> {
    let mut redirect_url = url::Url::parse(redirect_uri)
        .map_err(|_| AppError::BadRequest("Invalid redirect_uri".to_string()))?;

    redirect_url.query_pairs_mut().append_pair("error", error);
    redirect_url.query_pairs_mut().append_pair("error_description", error_description);
    if let Some(state_param) = state {
        redirect_url.query_pairs_mut().append_pair("state", state_param);
    }

    Ok(Redirect::to(&redirect_url.to_string()).into_response())
}

fn generate_consent_page(params: &AuthorizeParams, client: &OAuthClient) -> String {
    let scopes = params.scope.as_deref().unwrap_or("read");
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Authorize Application</title>
    <style>
        body {{ font-family: Arial, sans-serif; max-width: 500px; margin: 50px auto; padding: 20px; }}
        .consent-form {{ border: 1px solid #ccc; padding: 20px; border-radius: 5px; }}
        .buttons {{ margin-top: 20px; }}
        button {{ padding: 10px 20px; margin-right: 10px; border-radius: 3px; border: 1px solid #ccc; }}
        .approve {{ background: #4CAF50; color: white; }}
        .deny {{ background: #f44336; color: white; }}
    </style>
</head>
<body>
    <div class="consent-form">
        <h2>Authorize {}</h2>
        <p>The application <strong>{}</strong> is requesting access to your account with the following permissions:</p>
        <ul>
            <li>Scopes: {}</li>
        </ul>
        <p>Do you want to allow this access?</p>

        <form method="POST" action="/oauth/authorize">
            <input type="hidden" name="user_id" value="demo-user-id">
            <input type="hidden" name="client_id" value="{}">
            <input type="hidden" name="redirect_uri" value="{}">
            <input type="hidden" name="scope" value="{}">
            <input type="hidden" name="state" value="{}">
            <input type="hidden" name="code_challenge" value="{}">
            <input type="hidden" name="code_challenge_method" value="{}">

            <div class="buttons">
                <button type="submit" name="approved" value="true" class="approve">Approve</button>
                <button type="submit" name="approved" value="false" class="deny">Deny</button>
            </div>
        </form>
    </div>
</body>
</html>"#,
        client.name,
        client.name,
        scopes,
        params.client_id,
        params.redirect_uri,
        scopes,
        params.state.as_deref().unwrap_or(""),
        params.code_challenge.as_deref().unwrap_or(""),
        params.code_challenge_method.as_deref().unwrap_or(""),
    )
}