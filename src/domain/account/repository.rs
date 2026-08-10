use rust_decimal::Decimal;
use sqlx::MySqlPool;

use crate::error::AppError;

use super::model::AccountDto;

// `LEFT JOIN` so an account with zero transactions still returns a row (`SUM` of nothing is
// `NULL`, hence `COALESCE(..., 0)`). Grouping by `a.id` (the primary key) is enough for MySQL's
// `ONLY_FULL_GROUP_BY` to allow the other ungrouped `a.*` columns — they're functionally
// dependent on it.
const SELECT_FIELDS_WITH_BALANCE: &str = "a.id, a.name, a.user_id, a.starting_balance, \
    a.starting_balance + COALESCE(SUM(CASE WHEN t.category = 'INCOME' THEN t.amount ELSE -t.amount END), 0) AS current_balance \
    FROM accounts a LEFT JOIN transactions t ON t.account_id = a.id";

#[derive(sqlx::FromRow)]
struct AccountRow {
    id: i64,
    name: String,
    user_id: i64,
    starting_balance: Decimal,
    current_balance: Decimal,
}

impl From<AccountRow> for AccountDto {
    fn from(row: AccountRow) -> Self {
        AccountDto {
            id: row.id,
            name: row.name,
            user_id: row.user_id,
            starting_balance: row.starting_balance,
            current_balance: row.current_balance,
        }
    }
}

pub async fn find_all_for_user(
    pool: &MySqlPool,
    user_id: i64,
) -> Result<Vec<AccountDto>, AppError> {
    let accounts = sqlx::query_as::<_, AccountRow>(&format!(
        "SELECT {SELECT_FIELDS_WITH_BALANCE} WHERE a.user_id = ? GROUP BY a.id"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(accounts.into_iter().map(AccountDto::from).collect())
}

pub async fn find_by_id(pool: &MySqlPool, id: i64) -> Result<AccountDto, AppError> {
    let account = sqlx::query_as::<_, AccountRow>(&format!(
        "SELECT {SELECT_FIELDS_WITH_BALANCE} WHERE a.id = ? GROUP BY a.id"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(account.into())
}

pub async fn find_by_id_for_user(
    pool: &MySqlPool,
    id: i64,
    user_id: i64,
) -> Result<AccountDto, AppError> {
    let account = sqlx::query_as::<_, AccountRow>(&format!(
        "SELECT {SELECT_FIELDS_WITH_BALANCE} WHERE a.id = ? AND a.user_id = ? GROUP BY a.id"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(account.into())
}

pub async fn create(
    pool: &MySqlPool,
    name: &str,
    starting_balance: Decimal,
    user_id: i64,
) -> Result<AccountDto, AppError> {
    let result = sqlx::query(
        "INSERT INTO accounts (name, starting_balance, user_id) VALUES (?, ?, ?)",
    )
    .bind(name)
    .bind(starting_balance)
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
    starting_balance: Decimal,
) -> Result<AccountDto, AppError> {
    let result = sqlx::query(
        "UPDATE accounts SET name = ?, starting_balance = ? WHERE id = ? AND user_id = ?",
    )
    .bind(name)
    .bind(starting_balance)
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
