use rust_decimal::Decimal;
use sqlx::MySqlPool;

use crate::{domain::transaction::model::TransactionCategory, error::AppError};

use super::model::BudgetDto;

pub async fn find_all_for_user(
    pool: &MySqlPool,
    user_id: i64,
) -> Result<Vec<BudgetDto>, AppError> {
    let budgets = sqlx::query_as::<_, BudgetDto>(
        "SELECT category, monthly_limit FROM budgets WHERE user_id = ? ORDER BY category",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(budgets)
}

pub async fn find_for_user(
    pool: &MySqlPool,
    user_id: i64,
    category: TransactionCategory,
) -> Result<BudgetDto, AppError> {
    sqlx::query_as::<_, BudgetDto>(
        "SELECT category, monthly_limit FROM budgets WHERE user_id = ? AND category = ?",
    )
    .bind(user_id)
    .bind(category)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

/// Creates the caller's budget for `category`, or replaces its limit if one already exists —
/// `(user_id, category)` is unique, so `ON DUPLICATE KEY UPDATE` makes this a single upsert.
pub async fn upsert(
    pool: &MySqlPool,
    user_id: i64,
    category: TransactionCategory,
    monthly_limit: Decimal,
) -> Result<BudgetDto, AppError> {
    sqlx::query(
        "INSERT INTO budgets (user_id, category, monthly_limit) VALUES (?, ?, ?) \
         ON DUPLICATE KEY UPDATE monthly_limit = VALUES(monthly_limit)",
    )
    .bind(user_id)
    .bind(category)
    .bind(monthly_limit)
    .execute(pool)
    .await?;

    find_for_user(pool, user_id, category).await
}

pub async fn delete(
    pool: &MySqlPool,
    user_id: i64,
    category: TransactionCategory,
) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM budgets WHERE user_id = ? AND category = ?")
        .bind(user_id)
        .bind(category)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(())
}
