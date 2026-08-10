use axum::{extract::Path, extract::State, Json};

use crate::{error::AppError, state::AppState};

use super::model::{Transaction, TransactionListResponse, TransactionResponse};

pub async fn list(
    State(_state): State<AppState>,
) -> Result<Json<TransactionListResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn create(
    State(_state): State<AppState>,
    Json(_payload): Json<Transaction>,
) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}

pub async fn get_by_id(
    State(_state): State<AppState>,
    Path(_id): Path<i64>,
) -> Result<Json<TransactionResponse>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn update(
    State(_state): State<AppState>,
    Path(_id): Path<i64>,
    Json(_payload): Json<Transaction>,
) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}

pub async fn delete(
    State(_state): State<AppState>,
    Path(_id): Path<i64>,
) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}
