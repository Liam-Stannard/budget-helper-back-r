use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("resource not found")]
    NotFound,
    #[error("unauthorized")]
    Unauthorized,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("token error")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("password hashing error: {0}")]
    PasswordHash(String),
    #[error("not implemented")]
    NotImplemented,
}

#[derive(Serialize)]
struct ErrorBody {
    message: String,
    status: u16,
    timestamp: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Database(_) | AppError::Jwt(_) | AppError::PasswordHash(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::NotImplemented => StatusCode::NOT_IMPLEMENTED,
        };

        let body = ErrorBody {
            message: self.to_string(),
            status: status.as_u16(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        (status, Json(body)).into_response()
    }
}
