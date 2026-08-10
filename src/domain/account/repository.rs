use sqlx::MySqlPool;

use crate::error::AppError;

use super::model::Account;

pub async fn find_all(_pool: &MySqlPool) -> Result<Vec<Account>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn find_by_id(_pool: &MySqlPool, _id: i64) -> Result<Account, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn create(_pool: &MySqlPool, _account: &Account) -> Result<Account, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn update(_pool: &MySqlPool, _id: i64, _account: &Account) -> Result<Account, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn delete(_pool: &MySqlPool, _id: i64) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}
