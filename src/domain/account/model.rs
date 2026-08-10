use serde::{Deserialize, Serialize};

#[derive(sqlx::FromRow, Serialize, Debug, Clone)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub user_id: i64,
}

// The frontend sends just `{ "name": "..." }` on create/update (id and user_id are server-
// assigned/derived from the caller), so create/update take this instead of `Account` directly.
#[derive(Deserialize, Debug, Clone)]
pub struct AccountRequest {
    pub name: String,
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
