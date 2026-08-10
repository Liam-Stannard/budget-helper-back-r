use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};

use crate::{
    domain::{auth::jwt, token},
    error::AppError,
    state::AppState,
};

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    let claims = jwt::decode_claims(token, &state.config.jwt_secret)?;

    let stored_token = token::repository::find_by_token(&state.db, token)
        .await
        .map_err(|_| AppError::Unauthorized)?;
    if stored_token.expired || stored_token.revoked {
        return Err(AppError::Unauthorized);
    }

    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}
