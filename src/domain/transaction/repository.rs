use sqlx::MySqlPool;

use crate::domain::account::model::Account;
use crate::error::AppError;

use super::model::{Transaction, TransactionCategory, TransactionDto};

const SELECT_FIELDS: &str = "t.id, t.category, t.title, t.amount, t.date, t.account_id, t.user_id, a.name AS account_name \
    FROM transactions t JOIN accounts a ON a.id = t.account_id";

#[derive(sqlx::FromRow)]
struct TransactionRow {
    id: i64,
    category: TransactionCategory,
    title: String,
    amount: rust_decimal::Decimal,
    date: chrono::NaiveDate,
    account_id: i64,
    user_id: i64,
    account_name: String,
}

impl From<TransactionRow> for TransactionDto {
    fn from(row: TransactionRow) -> Self {
        TransactionDto {
            id: row.id,
            category: row.category,
            title: row.title,
            amount: row.amount,
            date: row.date,
            account: Account {
                id: row.account_id,
                name: row.account_name,
                user_id: row.user_id,
            },
        }
    }
}

pub async fn find_all_for_user(
    pool: &MySqlPool,
    user_id: i64,
) -> Result<Vec<TransactionDto>, AppError> {
    let transactions =
        sqlx::query_as::<_, TransactionRow>(&format!("SELECT {SELECT_FIELDS} WHERE t.user_id = ?"))
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(transactions.into_iter().map(TransactionDto::from).collect())
}

pub async fn find_by_id_for_user(
    pool: &MySqlPool,
    id: i64,
    user_id: i64,
) -> Result<TransactionDto, AppError> {
    let transaction = sqlx::query_as::<_, TransactionRow>(&format!(
        "SELECT {SELECT_FIELDS} WHERE t.id = ? AND t.user_id = ?"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(transaction.into())
}

async fn find_by_id(pool: &MySqlPool, id: i64) -> Result<TransactionDto, AppError> {
    let transaction =
        sqlx::query_as::<_, TransactionRow>(&format!("SELECT {SELECT_FIELDS} WHERE t.id = ?"))
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or(AppError::NotFound)?;
    Ok(transaction.into())
}

pub async fn create(
    pool: &MySqlPool,
    transaction: &Transaction,
) -> Result<TransactionDto, AppError> {
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
) -> Result<TransactionDto, AppError> {
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
