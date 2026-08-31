use sqlx::MySqlPool;

use crate::domain::account::model::Account;
use crate::error::AppError;

use super::model::{
    SortDirection, Transaction, TransactionCategory, TransactionDto, TransactionSortField,
};

/// Sentinels for an unbounded `from`/`to`, rather than `chrono::NaiveDate::MIN`/`MAX`: those are
/// far outside MySQL's `DATE` column range ('1000-01-01' to '9999-12-31') and would fail to
/// bind.
fn min_date() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(1000, 1, 1).expect("valid date literal")
}

fn max_date() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(9999, 12, 31).expect("valid date literal")
}

pub struct TransactionPage {
    pub transactions: Vec<TransactionDto>,
    pub total_elements: i64,
}

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

#[allow(clippy::too_many_arguments)]
pub async fn find_page_for_user(
    pool: &MySqlPool,
    user_id: i64,
    from: Option<chrono::NaiveDate>,
    to: Option<chrono::NaiveDate>,
    category: Option<TransactionCategory>,
    account_id: Option<i64>,
    sort: TransactionSortField,
    direction: SortDirection,
    page: u32,
    size: u32,
) -> Result<TransactionPage, AppError> {
    let from = from.unwrap_or_else(min_date);
    let to = to.unwrap_or_else(max_date);
    let offset = i64::from(page) * i64::from(size);
    let column = sort.column();
    let direction = direction.as_sql();

    // Built up conditionally (never with interpolated values — those are always `.bind()`ed
    // below) so the two filters stay independently optional and are kept in sync between the
    // `SELECT` and `COUNT(*)` queries.
    let mut filters = String::new();
    if category.is_some() {
        filters.push_str(" AND t.category = ?");
    }
    if account_id.is_some() {
        filters.push_str(" AND t.account_id = ?");
    }

    let transactions_sql = format!(
        "SELECT {SELECT_FIELDS} WHERE t.user_id = ? AND t.date BETWEEN ? AND ?{filters} \
         ORDER BY {column} {direction} LIMIT ? OFFSET ?"
    );
    let mut transactions_query = sqlx::query_as::<_, TransactionRow>(&transactions_sql)
        .bind(user_id)
        .bind(from)
        .bind(to);
    if let Some(category) = category {
        transactions_query = transactions_query.bind(category);
    }
    if let Some(account_id) = account_id {
        transactions_query = transactions_query.bind(account_id);
    }
    let transactions = transactions_query
        .bind(size)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let count_sql = format!(
        "SELECT COUNT(*) FROM transactions t WHERE t.user_id = ? AND t.date BETWEEN ? AND ?{filters}"
    );
    let mut count_query = sqlx::query_scalar(&count_sql)
        .bind(user_id)
        .bind(from)
        .bind(to);
    if let Some(category) = category {
        count_query = count_query.bind(category);
    }
    if let Some(account_id) = account_id {
        count_query = count_query.bind(account_id);
    }
    let total_elements: i64 = count_query.fetch_one(pool).await?;

    Ok(TransactionPage {
        transactions: transactions.into_iter().map(TransactionDto::from).collect(),
        total_elements,
    })
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
