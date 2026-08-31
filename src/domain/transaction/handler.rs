use axum::{
    extract::{Path, Query},
    extract::State,
    Json,
};

use crate::{domain::account, error::AppError, extractors::CurrentUser, state::AppState};

use super::{
    model::{
        SortDirection, Transaction, TransactionListQuery, TransactionListResponse,
        TransactionRequest, TransactionResponse, TransactionSortField,
    },
    repository,
};

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAX_PAGE_SIZE: u32 = 200;
const DEFAULT_SORT: (TransactionSortField, SortDirection) =
    (TransactionSortField::Date, SortDirection::Desc);

/// `"<field>,<direction>"`, e.g. `"date,desc"` — see `TransactionSortField`/`SortDirection` for
/// the allowed values of each half.
fn parse_sort(raw: &str) -> Result<(TransactionSortField, SortDirection), AppError> {
    let (field, direction) = raw
        .split_once(',')
        .ok_or_else(|| AppError::BadRequest(format!("invalid `sort` value: {raw}")))?;
    let field = field.parse::<TransactionSortField>().map_err(AppError::BadRequest)?;
    let direction = direction
        .parse::<SortDirection>()
        .map_err(AppError::BadRequest)?;
    Ok((field, direction))
}

async fn resolve_account_for_user(
    pool: &sqlx::MySqlPool,
    account_id: i64,
    user_id: i64,
) -> Result<(), AppError> {
    let existing_account = account::repository::find_by_id(pool, account_id).await?;
    if existing_account.user_id != user_id {
        return Err(AppError::Forbidden(
            "Account does not belong to the current user".to_string(),
        ));
    }
    Ok(())
}

pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<TransactionListQuery>,
) -> Result<Json<TransactionListResponse>, AppError> {
    if let (Some(from), Some(to)) = (query.from, query.to) {
        if from > to {
            return Err(AppError::BadRequest(
                "`from` must not be after `to`".to_string(),
            ));
        }
    }

    let size = query.size.unwrap_or(DEFAULT_PAGE_SIZE);
    if size == 0 || size > MAX_PAGE_SIZE {
        return Err(AppError::BadRequest(format!(
            "`size` must be between 1 and {MAX_PAGE_SIZE}"
        )));
    }
    let page = query.page.unwrap_or(0);

    let (sort, direction) = match query.sort.as_deref() {
        Some(raw) => parse_sort(raw)?,
        None => DEFAULT_SORT,
    };

    // Same ownership check `create`/`update` apply to a body-supplied `account.id`: filtering by
    // an account the user doesn't own is treated as forbidden, not as an empty result.
    if let Some(account_id) = query.account_id {
        resolve_account_for_user(&state.db, account_id, user.id).await?;
    }

    let result = repository::find_page_for_user(
        &state.db, user.id, query.from, query.to, query.category, query.account_id, sort,
        direction, page, size,
    )
    .await?;

    let total_pages = (result.total_elements + i64::from(size) - 1) / i64::from(size);

    Ok(Json(TransactionListResponse {
        transaction_list: result.transactions,
        total_elements: result.total_elements,
        total_pages,
    }))
}

pub async fn create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(payload): Json<TransactionRequest>,
) -> Result<Json<TransactionResponse>, AppError> {
    resolve_account_for_user(&state.db, payload.account.id, user.id).await?;
    let transaction = Transaction {
        id: 0,
        category: payload.category,
        title: payload.title,
        amount: payload.amount,
        date: payload.date,
        account_id: payload.account.id,
        user_id: user.id,
    };
    let created = repository::create(&state.db, &transaction).await?;
    Ok(Json(TransactionResponse {
        transaction: created,
    }))
}

pub async fn get_by_id(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<TransactionResponse>, AppError> {
    let transaction = repository::find_by_id_for_user(&state.db, id, user.id).await?;
    Ok(Json(TransactionResponse { transaction }))
}

pub async fn update(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<i64>,
    Json(payload): Json<TransactionRequest>,
) -> Result<Json<TransactionResponse>, AppError> {
    resolve_account_for_user(&state.db, payload.account.id, user.id).await?;
    let transaction = Transaction {
        id,
        category: payload.category,
        title: payload.title,
        amount: payload.amount,
        date: payload.date,
        account_id: payload.account.id,
        user_id: user.id,
    };
    let updated = repository::update(&state.db, id, user.id, &transaction).await?;
    Ok(Json(TransactionResponse {
        transaction: updated,
    }))
}

pub async fn delete(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<(), AppError> {
    repository::delete(&state.db, id, user.id).await
}
