use sqlx::MySqlPool;

use crate::error::AppError;

use super::model::Transaction;

const SELECT_FIELDS: &str =
    "id, category, title, amount, date, account_id, user_id FROM transactions";

pub async fn find_all_for_user(
    pool: &MySqlPool,
    user_id: i64,
) -> Result<Vec<Transaction>, AppError> {
    let transactions =
        sqlx::query_as::<_, Transaction>(&format!("SELECT {SELECT_FIELDS} WHERE user_id = ?"))
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(transactions)
}

pub async fn find_by_id_for_user(
    pool: &MySqlPool,
    id: i64,
    user_id: i64,
) -> Result<Transaction, AppError> {
    let transaction = sqlx::query_as::<_, Transaction>(&format!(
        "SELECT {SELECT_FIELDS} WHERE id = ? AND user_id = ?"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(transaction)
}

async fn find_by_id(pool: &MySqlPool, id: i64) -> Result<Transaction, AppError> {
    let transaction =
        sqlx::query_as::<_, Transaction>(&format!("SELECT {SELECT_FIELDS} WHERE id = ?"))
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or(AppError::NotFound)?;
    Ok(transaction)
}

pub async fn create(pool: &MySqlPool, transaction: &Transaction) -> Result<Transaction, AppError> {
    let result = sqlx::query(
        "INSERT INTO transactions (category, title, amount, date, account_id, user_id) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(transaction.category)
    .bind(&transaction.title)
    .bind(transaction.amount)
    .bind(transaction.date)
    .bind(transaction.account_id)
    .bind(transaction.user_id)
    .execute(pool)
    .await?;

    find_by_id(pool, result.last_insert_id() as i64).await
}

pub async fn update(
    pool: &MySqlPool,
    id: i64,
    user_id: i64,
    transaction: &Transaction,
) -> Result<Transaction, AppError> {
    let result = sqlx::query(
        "UPDATE transactions SET category = ?, title = ?, amount = ?, date = ?, account_id = ? WHERE id = ? AND user_id = ?",
    )
    .bind(transaction.category)
    .bind(&transaction.title)
    .bind(transaction.amount)
    .bind(transaction.date)
    .bind(transaction.account_id)
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
    let result = sqlx::query("DELETE FROM transactions WHERE id = ? AND user_id = ?")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(())
}
