use axum::{extract::State, http::HeaderMap, Json};

use crate::{
    domain::{token, user},
    error::AppError,
    state::AppState,
};

use super::{
    jwt,
    model::{AuthenticationRequest, AuthenticationResponse, Claims, RegisterRequest},
};

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<AuthenticationResponse>, AppError> {
    let hashed = user::password::hash_password(&payload.password)?;

    let new_user = user::model::User {
        id: 0,
        username: payload.username,
        role: user::model::Role::RoleUser,
        email: payload.email,
        password: hashed,
    };
    let created = user::repository::create(&state.db, &new_user).await?;

    let access = jwt::generate_access_token(&created.username, &state.config)?;
    let refresh = jwt::generate_refresh_token(&created.username, &state.config)?;

    token::repository::revoke_all_for_user(&state.db, created.id).await?;
    token::repository::create(&state.db, &access, created.id).await?;

    Ok(Json(AuthenticationResponse { access, refresh }))
}

pub async fn authenticate(
    State(state): State<AppState>,
    Json(payload): Json<AuthenticationRequest>,
) -> Result<Json<AuthenticationResponse>, AppError> {
    let existing = user::repository::find_by_username(&state.db, &payload.username).await?;

    if !user::password::verify_password(&payload.password, &existing.password)? {
        return Err(AppError::Unauthorized);
    }

    let access = jwt::generate_access_token(&existing.username, &state.config)?;
    let refresh = jwt::generate_refresh_token(&existing.username, &state.config)?;

    token::repository::revoke_all_for_user(&state.db, existing.id).await?;
    token::repository::create(&state.db, &access, existing.id).await?;

    Ok(Json(AuthenticationResponse { access, refresh }))
}

pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthenticationResponse>, AppError> {
    let refresh_token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    let claims: Claims = jwt::decode_claims(refresh_token, &state.config.jwt_secret)?;
    let existing = user::repository::find_by_username(&state.db, &claims.sub).await?;

    let access = jwt::generate_access_token(&existing.username, &state.config)?;

    token::repository::revoke_all_for_user(&state.db, existing.id).await?;
    token::repository::create(&state.db, &access, existing.id).await?;

    Ok(Json(AuthenticationResponse {
        access,
        refresh: refresh_token.to_string(),
    }))
}
