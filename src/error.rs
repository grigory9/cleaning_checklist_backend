use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::Serialize;
use thiserror::Error;
use std::io;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("user not found")]
    UserNotFound,
    #[error("validation error: {0}")]
    Validation(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("email already exists")]
    EmailAlreadyExists,
    #[error("invalid token")]
    InvalidToken,
    #[error("invalid request")]
    InvalidRequest,
    #[error("invalid grant type")]
    InvalidGrantType,
    #[error("invalid scope")]
    InvalidScope,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    AxumJsonRejection(#[from] axum::extract::rejection::JsonRejection),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            AppError::UserNotFound => (StatusCode::NOT_FOUND, "user_not_found"),
            AppError::Validation(_) => (StatusCode::BAD_REQUEST, "validation_error"),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            AppError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "invalid_credentials"),
            AppError::EmailAlreadyExists => (StatusCode::CONFLICT, "email_already_exists"),
            AppError::InvalidToken => (StatusCode::UNAUTHORIZED, "invalid_token"),
            AppError::InvalidRequest => (StatusCode::BAD_REQUEST, "invalid_request"),
            AppError::InvalidGrantType => (StatusCode::BAD_REQUEST, "invalid_grant_type"),
            AppError::InvalidScope => (StatusCode::FORBIDDEN, "invalid_scope"),
            AppError::Sqlx(_) => (StatusCode::INTERNAL_SERVER_ERROR, "db_error"),
            AppError::AxumJsonRejection(_) => (StatusCode::BAD_REQUEST, "invalid_json"),
            AppError::Other(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
            AppError::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, "io_error"),
        };
        let message = self.to_string();
        (status, Json(ErrorBody{ code, message })).into_response()
    }
}
