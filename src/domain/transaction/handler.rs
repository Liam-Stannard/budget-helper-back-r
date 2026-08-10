use axum::{extract::Path, extract::State, Json};

use crate::{domain::account, error::AppError, extractors::CurrentUser, state::AppState};

use super::{
    model::{Transaction, TransactionListResponse, TransactionResponse},
    repository,
};

async fn resolve_account_for_user(
    pool: &sqlx::MySqlPool,
    account_id: i64,
    user_id: i64,
) -> Result<(), AppError> {
    let existing_account = account::repository::find_by_id(pool, account_id).await?;
    if existing_account.user_id != user_id {
        return Err(AppError::Forbidden(
            "Account does not belong to the current user".to_string(),
        ));
    }
    Ok(())
}

pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<TransactionListResponse>, AppError> {
    let transaction_list = repository::find_all_for_user(&state.db, user.id).await?;
    Ok(Json(TransactionListResponse { transaction_list }))
}

pub async fn create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(mut payload): Json<Transaction>,
) -> Result<(), AppError> {
    resolve_account_for_user(&state.db, payload.account_id, user.id).await?;
    payload.user_id = user.id;
    repository::create(&state.db, &payload).await?;
    Ok(())
}

pub async fn get_by_id(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<TransactionResponse>, AppError> {
    let transaction = repository::find_by_id_for_user(&state.db, id, user.id).await?;
    Ok(Json(TransactionResponse { transaction }))
}

pub async fn update(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<i64>,
    Json(mut payload): Json<Transaction>,
) -> Result<(), AppError> {
    resolve_account_for_user(&state.db, payload.account_id, user.id).await?;
    payload.user_id = user.id;
    repository::update(&state.db, id, user.id, &payload).await?;
    Ok(())
}

pub async fn delete(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<(), AppError> {
    repository::delete(&state.db, id, user.id).await
}
