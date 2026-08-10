use axum::{extract::Path, extract::State, Json};

use crate::{error::AppError, state::AppState};

use super::{model::User, password, repository};

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<User>>, AppError> {
    let users = repository::find_all(&state.db).await?;
    Ok(Json(users))
}

pub async fn create(
    State(state): State<AppState>,
    Json(mut payload): Json<User>,
) -> Result<Json<User>, AppError> {
    payload.password = password::hash_password(&payload.password)?;
    let user = repository::create(&state.db, &payload).await?;
    Ok(Json(user))
}

pub async fn get_by_id(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<User>, AppError> {
    let user = repository::find_by_id(&state.db, id).await?;
    Ok(Json(user))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(mut payload): Json<User>,
) -> Result<Json<User>, AppError> {
    payload.password = password::hash_password(&payload.password)?;
    let user = repository::update(&state.db, id, &payload).await?;
    Ok(Json(user))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> Result<(), AppError> {
    repository::delete(&state.db, id).await
}
