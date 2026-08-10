use sqlx::MySqlPool;

use crate::error::AppError;

use super::model::User;

pub async fn find_all(_pool: &MySqlPool) -> Result<Vec<User>, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn find_by_id(_pool: &MySqlPool, _id: i64) -> Result<User, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn create(_pool: &MySqlPool, _user: &User) -> Result<User, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn update(_pool: &MySqlPool, _id: i64, _user: &User) -> Result<User, AppError> {
    Err(AppError::NotImplemented)
}

pub async fn delete(_pool: &MySqlPool, _id: i64) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}
