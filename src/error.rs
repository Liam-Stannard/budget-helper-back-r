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
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("token error")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("password hashing error: {0}")]
    PasswordHash(String),
}

#[derive(Serialize)]
struct ErrorBody {
    message: String,
    status: u16,
    timestamp: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!(error = ?self, "request failed");

        let status = match &self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Database(e) => {
                let is_duplicate = e
                    .as_database_error()
                    .and_then(|de| de.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
                    .is_some_and(|de| de.number() == 1062);
                if is_duplicate {
                    StatusCode::CONFLICT
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            }
            AppError::Jwt(_) | AppError::PasswordHash(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let message = if status == StatusCode::CONFLICT && matches!(self, AppError::Database(_)) {
            "resource already exists".to_string()
        } else {
            self.to_string()
        };

        let body = ErrorBody {
            message,
            status: status.as_u16(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        (status, Json(body)).into_response()
    }
}
