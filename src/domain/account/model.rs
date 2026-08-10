use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// The nested shape used when an account is embedded inside another response (currently
// `TransactionDto.account`) — identity only, no balance. Computing a balance there would mean
// an extra aggregate per transaction row for a value nothing reads at that call site.
#[derive(sqlx::FromRow, Serialize, Debug, Clone)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub user_id: i64,
}

// The `/api/v1/account*` response shape: adds `current_balance`, computed as
// `starting_balance` plus every transaction against the account (`INCOME` credits, everything
// else debits) — see `repository::SELECT_FIELDS_WITH_BALANCE`. Never stored, so it can't drift
// from the transaction history the way a persisted running balance could.
#[derive(Serialize, Debug, Clone)]
pub struct AccountDto {
    pub id: i64,
    pub name: String,
    pub user_id: i64,
    #[serde(rename = "startingBalance")]
    pub starting_balance: Decimal,
    #[serde(rename = "currentBalance")]
    pub current_balance: Decimal,
}

// The frontend sends `{ "name": "...", "startingBalance": "..." }` on create/update (id and
// user_id are server-assigned/derived from the caller). `startingBalance` defaults to 0 so
// existing callers that only send `name` keep working.
#[derive(Deserialize, Debug, Clone)]
pub struct AccountRequest {
    pub name: String,
    #[serde(rename = "startingBalance", default)]
    pub starting_balance: Decimal,
}

#[derive(Serialize, Debug, Clone)]
pub struct AccountListResponse {
    #[serde(rename = "accountList")]
    pub account_list: Vec<AccountDto>,
}

#[derive(Serialize, Debug, Clone)]
pub struct AccountResponse {
    pub account: AccountDto,
}
