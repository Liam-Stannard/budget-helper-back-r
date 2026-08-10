use sqlx::MySqlPool;

use crate::error::AppError;

use super::model::Account;

const SELECT_FIELDS: &str = "id, name, user_id FROM accounts";

pub async fn find_all_for_user(pool: &MySqlPool, user_id: i64) -> Result<Vec<Account>, AppError> {
    let accounts =
        sqlx::query_as::<_, Account>(&format!("SELECT {SELECT_FIELDS} WHERE user_id = ?"))
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(accounts)
}

pub async fn find_by_id(pool: &MySqlPool, id: i64) -> Result<Account, AppError> {
    let account = sqlx::query_as::<_, Account>(&format!("SELECT {SELECT_FIELDS} WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(account)
}

pub async fn find_by_id_for_user(
    pool: &MySqlPool,
    id: i64,
    user_id: i64,
) -> Result<Account, AppError> {
    let account = sqlx::query_as::<_, Account>(&format!(
        "SELECT {SELECT_FIELDS} WHERE id = ? AND user_id = ?"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(account)
}

pub async fn create(pool: &MySqlPool, name: &str, user_id: i64) -> Result<Account, AppError> {
    let result = sqlx::query("INSERT INTO accounts (name, user_id) VALUES (?, ?)")
        .bind(name)
        .bind(user_id)
        .execute(pool)
        .await?;

    find_by_id(pool, result.last_insert_id() as i64).await
}

pub async fn update(
    pool: &MySqlPool,
    id: i64,
    user_id: i64,
    name: &str,
) -> Result<Account, AppError> {
    let result = sqlx::query("UPDATE accounts SET name = ? WHERE id = ? AND user_id = ?")
        .bind(name)
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    find_by_id(pool, id).await
}

pub async fn delete(pool: &MySqlPool, id: i64, user_id: i64) -> Result<(), AppError> {
    find_by_id_for_user(pool, id, user_id).await?;

    let has_transactions: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM transactions WHERE account_id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

    if has_transactions.is_some() {
        return Err(AppError::Conflict(
            "Cannot delete an account that still has transactions".to_string(),
        ));
    }

    sqlx::query("DELETE FROM accounts WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}
