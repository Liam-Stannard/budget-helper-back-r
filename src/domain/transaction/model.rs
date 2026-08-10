use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(sqlx::Type, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[sqlx(rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionCategory {
    Food,
    Bill,
    Subscription,
    Clothing,
}

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub id: i64,
    pub category: TransactionCategory,
    pub title: String,
    pub amount: Decimal,
    pub date: NaiveDate,
    pub account_id: i64,
    pub user_id: i64,
}

#[derive(Serialize, Debug, Clone)]
pub struct TransactionListResponse {
    #[serde(rename = "transactionList")]
    pub transaction_list: Vec<Transaction>,
}

#[derive(Serialize, Debug, Clone)]
pub struct TransactionResponse {
    pub transaction: Transaction,
}
