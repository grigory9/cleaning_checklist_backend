use crate::auth::scopes::ScopeSet;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TokenError {
    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),
    #[error("Token expired")]
    Expired,
    #[error("Invalid token")]
    Invalid,
    #[error("Missing secret key")]
    MissingSecret,
}

pub type TokenResult<T> = Result<T, TokenError>;

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub sub: String,              // user_id or client_id for client credentials
    pub client_id: String,        // OAuth client ID
    pub scopes: String,          // space-separated scopes
    pub token_type: String,      // "access_token"
    pub iat: i64,                // issued at
    pub exp: i64,                // expires at
    pub jti: String,             // token ID (for revocation)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshTokenClaims {
    pub sub: String,              // user_id or client_id
    pub client_id: String,        // OAuth client ID
    pub scopes: String,          // space-separated scopes
    pub token_type: String,      // "refresh_token"
    pub iat: i64,                // issued at
    pub exp: i64,                // expires at
    pub jti: String,             // token ID (for revocation)
}

pub struct TokenGenerator {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl TokenGenerator {
    pub fn new() -> TokenResult<Self> {
        let secret = env::var("JWT_SECRET")
            .or_else(|_| env::var("SECRET_KEY"))
            .map_err(|_| TokenError::MissingSecret)?;

        let encoding_key = EncodingKey::from_secret(secret.as_ref());
        let decoding_key = DecodingKey::from_secret(secret.as_ref());

        Ok(Self {
            encoding_key,
            decoding_key,
        })
    }

    pub fn generate_access_token(
        &self,
        user_id: Option<&str>,
        client_id: &str,
        scopes: &ScopeSet,
        expires_in_minutes: i64,
    ) -> TokenResult<(String, String)> {
        let now = Utc::now();
        let exp = now + Duration::minutes(expires_in_minutes);
        let jti = generate_random_token(32);

        let claims = AccessTokenClaims {
            sub: user_id.unwrap_or(client_id).to_string(),
            client_id: client_id.to_string(),
            scopes: scopes.to_string(),
            token_type: "access_token".to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            jti: jti.clone(),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)?;
        Ok((token, jti))
    }

    pub fn generate_refresh_token(
        &self,
        user_id: Option<&str>,
        client_id: &str,
        scopes: &ScopeSet,
        expires_in_days: i64,
    ) -> TokenResult<(String, String)> {
        let now = Utc::now();
        let exp = now + Duration::days(expires_in_days);
        let jti = generate_random_token(32);

        let claims = RefreshTokenClaims {
            sub: user_id.unwrap_or(client_id).to_string(),
            client_id: client_id.to_string(),
            scopes: scopes.to_string(),
            token_type: "refresh_token".to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            jti: jti.clone(),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)?;
        Ok((token, jti))
    }

    pub fn validate_access_token(&self, token: &str) -> TokenResult<AccessTokenClaims> {
        let token_data: TokenData<AccessTokenClaims> = decode(
            token,
            &self.decoding_key,
            &Validation::default(),
        )?;

        let claims = token_data.claims;
        if claims.token_type != "access_token" {
            return Err(TokenError::Invalid);
        }

        if claims.exp < Utc::now().timestamp() {
            return Err(TokenError::Expired);
        }

        Ok(claims)
    }

    pub fn validate_refresh_token(&self, token: &str) -> TokenResult<RefreshTokenClaims> {
        let token_data: TokenData<RefreshTokenClaims> = decode(
            token,
            &self.decoding_key,
            &Validation::default(),
        )?;

        let claims = token_data.claims;
        if claims.token_type != "refresh_token" {
            return Err(TokenError::Invalid);
        }

        if claims.exp < Utc::now().timestamp() {
            return Err(TokenError::Expired);
        }

        Ok(claims)
    }
}

pub fn generate_random_token(length: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..62);
            match idx {
                0..=25 => (b'a' + idx) as char,
                26..=51 => (b'A' + (idx - 26)) as char,
                _ => (b'0' + (idx - 52)) as char,
            }
        })
        .collect()
}

pub fn generate_authorization_code() -> String {
    generate_random_token(43) // URL-safe, 43 chars = ~256 bits entropy
}

pub fn generate_client_secret() -> String {
    generate_random_token(64)
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn generate_state() -> String {
    generate_random_token(32)
}

pub fn generate_code_verifier() -> String {
    generate_random_token(128)
}

pub fn generate_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}