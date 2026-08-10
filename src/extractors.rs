use axum::{extract::FromRequestParts, http::request::Parts, Extension};

use crate::{
    domain::auth::model::Claims,
    domain::user::{model::User, repository},
    error::AppError,
    state::AppState,
};

pub struct CurrentUser(pub User);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Extension(claims) = Extension::<Claims>::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::Unauthorized)?;

        let user = repository::find_by_username(&state.db, &claims.sub).await?;

        Ok(CurrentUser(user))
    }
}
