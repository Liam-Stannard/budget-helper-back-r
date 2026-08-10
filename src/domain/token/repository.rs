use sqlx::MySqlPool;

use crate::error::AppError;

use super::model::Token;

const SELECT_FIELDS: &str = "id, token, expired, revoked, user_id FROM token";

pub async fn create(pool: &MySqlPool, token: &str, user_id: i64) -> Result<Token, AppError> {
    let result = sqlx::query(
        "INSERT INTO token (token, expired, revoked, user_id) VALUES (?, FALSE, FALSE, ?)",
    )
    .bind(token)
    .bind(user_id)
    .execute(pool)
    .await?;

    let id = result.last_insert_id() as i32;
    let stored = sqlx::query_as::<_, Token>(&format!("SELECT {SELECT_FIELDS} WHERE id = ?"))
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(stored)
}

pub async fn find_by_token(pool: &MySqlPool, token: &str) -> Result<Token, AppError> {
    let stored = sqlx::query_as::<_, Token>(&format!("SELECT {SELECT_FIELDS} WHERE token = ?"))
        .bind(token)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(stored)
}

pub async fn revoke_all_for_user(pool: &MySqlPool, user_id: i64) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE token SET expired = TRUE, revoked = TRUE WHERE user_id = ? AND expired = FALSE AND revoked = FALSE",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}
