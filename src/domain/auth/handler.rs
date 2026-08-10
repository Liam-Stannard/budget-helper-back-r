use axum::{extract::State, Json};

use crate::{error::AppError, state::AppState};

use super::model::{AuthenticationRequest, AuthenticationResponse, RegisterRequest};

pub async fn register(
    State(_state): State<AppState>,
    Json(_payload): Json<RegisterRequest>,
) -> Result<Json<AuthenticationResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn authenticate(
    State(_state): State<AppState>,
    Json(_payload): Json<AuthenticationRequest>,
) -> Result<Json<AuthenticationResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn refresh(
    State(_state): State<AppState>,
) -> Result<Json<AuthenticationResponse>, AppError> {
    Err(AppError::NotImplemented)
}
