use axum::{extract::Path, extract::State, Json};

use crate::{error::AppError, extractors::CurrentUser, state::AppState};

use super::{
    model::{Account, AccountListResponse, AccountResponse},
    repository,
};

pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<AccountListResponse>, AppError> {
    let account_list = repository::find_all_for_user(&state.db, user.id).await?;
    Ok(Json(AccountListResponse { account_list }))
}

pub async fn create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(payload): Json<Account>,
) -> Result<(), AppError> {
    repository::create(&state.db, &payload.name, user.id).await?;
    Ok(())
}

pub async fn get_by_id(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<AccountResponse>, AppError> {
    let account = repository::find_by_id_for_user(&state.db, id, user.id).await?;
    Ok(Json(AccountResponse { account }))
}

pub async fn update(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<i64>,
    Json(payload): Json<Account>,
) -> Result<(), AppError> {
    repository::update(&state.db, id, user.id, &payload.name).await?;
    Ok(())
}

pub async fn delete(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<(), AppError> {
    repository::delete(&state.db, id, user.id).await
}
