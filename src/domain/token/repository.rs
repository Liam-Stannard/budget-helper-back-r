use sqlx::MySqlPool;

use crate::error::AppError;

use super::model::Token;

pub async fn create(_pool: &MySqlPool, _token: &Token) -> Result<Token, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn find_valid_tokens_by_user(
    _pool: &MySqlPool,
    _user_id: i64,
) -> Result<Vec<Token>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn find_by_token(_pool: &MySqlPool, _token: &str) -> Result<Token, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn revoke(_pool: &MySqlPool, _id: i32) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}
