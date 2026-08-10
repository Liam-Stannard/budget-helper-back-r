use serde::{Deserialize, Serialize};

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub user_id: i64,
}

#[derive(Serialize, Debug, Clone)]
pub struct AccountListResponse {
    #[serde(rename = "accountList")]
    pub account_list: Vec<Account>,
}

#[derive(Serialize, Debug, Clone)]
pub struct AccountResponse {
    pub account: Account,
}
