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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionCategory {
    Food,
    Bill,
    Subscription,
    Clothing,
}

impl TransactionCategory {
    fn as_str(&self) -> &'static str {
        match self {
            TransactionCategory::Food => "FOOD",
            TransactionCategory::Bill => "BILL",
            TransactionCategory::Subscription => "SUBSCRIPTION",
            TransactionCategory::Clothing => "CLOTHING",
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
