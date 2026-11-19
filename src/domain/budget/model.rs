use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::transaction::model::TransactionCategory;

// The wire shape of a budget: identity is `(caller, category)`, so neither the row `id` nor
// `user_id` is exposed — a client addresses a budget by its category (`/api/v1/budget/{category}`).
#[derive(sqlx::FromRow, Serialize, Debug, Clone)]
pub struct BudgetDto {
    pub category: TransactionCategory,
    #[serde(rename = "monthlyLimit")]
    pub monthly_limit: Decimal,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BudgetRequest {
    #[serde(rename = "monthlyLimit")]
    pub monthly_limit: Decimal,
}

#[derive(Serialize, Debug, Clone)]
pub struct BudgetListResponse {
    #[serde(rename = "budgetList")]
    pub budget_list: Vec<BudgetDto>,
}

#[derive(Serialize, Debug, Clone)]
pub struct BudgetResponse {
    pub budget: BudgetDto,
}
