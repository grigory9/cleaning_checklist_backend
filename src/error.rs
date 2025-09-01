use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("validation error: {0}")]
    Validation(String),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    AxumJsonRejection(#[from] axum::extract::rejection::JsonRejection),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
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
            AppError::Validation(_) => (StatusCode::BAD_REQUEST, "validation_error"),
            AppError::Sqlx(_) => (StatusCode::INTERNAL_SERVER_ERROR, "db_error"),
            AppError::AxumJsonRejection(_) => (StatusCode::BAD_REQUEST, "invalid_json"),
            AppError::Other(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };
        let message = self.to_string();
        (status, Json(ErrorBody{ code, message })).into_response()
    }
}
