use axum::{extract::Path, extract::State, http::StatusCode, Json};
use rust_decimal::Decimal;

use crate::{
    domain::transaction::model::TransactionCategory, error::AppError, extractors::CurrentUser,
    state::AppState,
};

use super::{
    model::{BudgetListResponse, BudgetRequest, BudgetResponse},
    repository,
};

/// Parses the `{category}` path segment. Only spending categories can carry a budget, so
/// `INCOME` is rejected alongside unknown values.
fn parse_budget_category(raw: &str) -> Result<TransactionCategory, AppError> {
    match raw.parse::<TransactionCategory>() {
        Ok(TransactionCategory::Income) => Err(AppError::BadRequest(
            "INCOME cannot have a budget".to_string(),
        )),
        Ok(category) => Ok(category),
        Err(message) => Err(AppError::BadRequest(message)),
    }
}

pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<BudgetListResponse>, AppError> {
    let budget_list = repository::find_all_for_user(&state.db, user.id).await?;
    Ok(Json(BudgetListResponse { budget_list }))
}

pub async fn upsert(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(category): Path<String>,
    Json(payload): Json<BudgetRequest>,
) -> Result<Json<BudgetResponse>, AppError> {
    let category = parse_budget_category(&category)?;

    if payload.monthly_limit <= Decimal::ZERO {
        return Err(AppError::BadRequest(
            "monthlyLimit must be greater than zero".to_string(),
        ));
    }

    let budget = repository::upsert(&state.db, user.id, category, payload.monthly_limit).await?;
    Ok(Json(BudgetResponse { budget }))
}

pub async fn delete(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(category): Path<String>,
) -> Result<StatusCode, AppError> {
    let category = parse_budget_category(&category)?;
    repository::delete(&state.db, user.id, category).await?;
    Ok(StatusCode::NO_CONTENT)
}
