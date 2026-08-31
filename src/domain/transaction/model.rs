use std::str::FromStr;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{
    encode::IsNull,
    error::BoxDynError,
    mysql::{MySql, MySqlTypeInfo, MySqlValueRef},
    Decode, Encode, Type,
};

use crate::domain::account::model::Account;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionCategory {
    Food,
    Bill,
    Subscription,
    Clothing,
    // Not a spending category like the others — see `repository::SELECT_FIELDS_WITH_BALANCE`
    // in the account domain, where it's the one category that credits an account's balance
    // instead of debiting it.
    Income,
}

impl TransactionCategory {
    fn as_str(&self) -> &'static str {
        match self {
            TransactionCategory::Food => "FOOD",
            TransactionCategory::Bill => "BILL",
            TransactionCategory::Subscription => "SUBSCRIPTION",
            TransactionCategory::Clothing => "CLOTHING",
            TransactionCategory::Income => "INCOME",
        }
    }
}

impl FromStr for TransactionCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "FOOD" => Ok(TransactionCategory::Food),
            "BILL" => Ok(TransactionCategory::Bill),
            "SUBSCRIPTION" => Ok(TransactionCategory::Subscription),
            "CLOTHING" => Ok(TransactionCategory::Clothing),
            "INCOME" => Ok(TransactionCategory::Income),
            other => Err(format!("unknown transaction category: {other}")),
        }
    }
}

// See the equivalent comment on `Role` in domain/user/model.rs: `category` is stored/compared
// as a plain VARCHAR and mapped manually rather than via `#[derive(sqlx::Type)]`, which targets
// MySQL's native ENUM type and can't match a generic VARCHAR column.
impl Type<MySql> for TransactionCategory {
    fn type_info() -> MySqlTypeInfo {
        <String as Type<MySql>>::type_info()
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        <String as Type<MySql>>::compatible(ty)
    }
}

impl Encode<'_, MySql> for TransactionCategory {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        <&str as Encode<MySql>>::encode_by_ref(&self.as_str(), buf)
    }
}

impl<'r> Decode<'r, MySql> for TransactionCategory {
    fn decode(value: MySqlValueRef<'r>) -> Result<Self, BoxDynError> {
        let s = <&str as Decode<MySql>>::decode(value)?;
        s.parse::<TransactionCategory>().map_err(Into::into)
    }
}

// The DB-facing row: `account_id` is the foreign key column. The API instead
// exchanges a nested `account` object (see `TransactionRequest`/`TransactionDto`
// below), matching what the frontend sends/expects.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: i64,
    pub category: TransactionCategory,
    pub title: String,
    pub amount: Decimal,
    pub date: NaiveDate,
    pub account_id: i64,
    pub user_id: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AccountRef {
    pub id: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TransactionRequest {
    pub category: TransactionCategory,
    pub title: String,
    pub amount: Decimal,
    pub date: NaiveDate,
    pub account: AccountRef,
}

#[derive(Serialize, Debug, Clone)]
pub struct TransactionDto {
    pub id: i64,
    pub category: TransactionCategory,
    pub title: String,
    pub amount: Decimal,
    pub date: NaiveDate,
    pub account: Account,
}

#[derive(Serialize, Debug, Clone)]
pub struct TransactionListResponse {
    #[serde(rename = "transactionList")]
    pub transaction_list: Vec<TransactionDto>,
    #[serde(rename = "totalElements")]
    pub total_elements: i64,
    #[serde(rename = "totalPages")]
    pub total_pages: i64,
}

/// Query params for `GET /api/v1/transaction`. All optional: an absent `from`/`to` leaves that
/// side of the date range unbounded, an absent `page`/`size`/`sort` falls back to the
/// handler's defaults (see `handler::list`), and an absent `category`/`account_id` leaves that
/// filter unapplied. `category`/`account_id` are independently optional and combinable.
#[derive(Deserialize, Debug, Clone)]
pub struct TransactionListQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub page: Option<u32>,
    pub size: Option<u32>,
    /// `"<field>,<direction>"`, e.g. `"date,desc"`. See `TransactionSortField`/`SortDirection`.
    pub sort: Option<String>,
    pub category: Option<TransactionCategory>,
    /// Filters to transactions on this account. Ownership is checked in `handler::list` (see
    /// `resolve_account_for_user`) before this reaches the repository.
    pub account_id: Option<i64>,
}

/// The columns the transactions list can be sorted by. Deliberately an allowlist rather than
/// accepting a raw column name: `sort` comes from the query string, and the SQL it drives is
/// built with `format!` (see `repository::SELECT_FIELDS`), which can't parameterize a column
/// name the way `.bind()` parameterizes a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionSortField {
    Title,
    Account,
    Amount,
    Date,
    Category,
}

impl TransactionSortField {
    pub fn column(&self) -> &'static str {
        match self {
            TransactionSortField::Title => "t.title",
            // `SELECT_FIELDS` already joins `accounts a`, so sorting by the account's name
            // needs no extra join.
            TransactionSortField::Account => "a.name",
            TransactionSortField::Amount => "t.amount",
            TransactionSortField::Date => "t.date",
            TransactionSortField::Category => "t.category",
        }
    }
}

impl FromStr for TransactionSortField {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "title" => Ok(TransactionSortField::Title),
            "account" => Ok(TransactionSortField::Account),
            "amount" => Ok(TransactionSortField::Amount),
            "date" => Ok(TransactionSortField::Date),
            "category" => Ok(TransactionSortField::Category),
            other => Err(format!("unknown sort field: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    pub fn as_sql(&self) -> &'static str {
        match self {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        }
    }
}

impl FromStr for SortDirection {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "asc" => Ok(SortDirection::Asc),
            "desc" => Ok(SortDirection::Desc),
            other => Err(format!("unknown sort direction: {other}")),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct TransactionResponse {
    pub transaction: TransactionDto,
}
