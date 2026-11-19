//! Contract tests for `/api/v1/budget*`. See docs/api-spec.md.

mod common;

use axum::http::StatusCode;
use common::{register_user, TestApp};
use serde::Deserialize;
use serde_json::json;

// Denies unknown fields: a budget is addressed by category, so neither `id` nor `user_id`
// should ever leak onto the wire.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct BudgetContract {
    category: String,
    #[serde(rename = "monthlyLimit")]
    monthly_limit: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BudgetResponseContract {
    budget: BudgetContract,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BudgetListResponseContract {
    #[serde(rename = "budgetList")]
    budget_list: Vec<BudgetContract>,
}

#[tokio::test]
async fn list_budgets_is_empty_for_a_new_user() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "budget-empty").await;

    let response = app.get("/api/v1/budget", Some(&token)).await;

    assert_eq!(response.status, StatusCode::OK);
    let body: BudgetListResponseContract = response.body_as();
    assert!(body.budget_list.is_empty());

    app.cleanup().await;
}

#[tokio::test]
async fn put_budget_creates_then_updates_and_list_is_ordered_by_category() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "budget-upsert").await;

    let created = app
        .put("/api/v1/budget/FOOD", Some(&token), json!({ "monthlyLimit": "300.00" }))
        .await;
    assert_eq!(created.status, StatusCode::OK);
    let body: BudgetResponseContract = created.body_as();
    assert_eq!(body.budget.category, "FOOD");
    assert_eq!(body.budget.monthly_limit, "300.0000");

    let updated = app
        .put("/api/v1/budget/FOOD", Some(&token), json!({ "monthlyLimit": "275.5" }))
        .await;
    assert_eq!(updated.status, StatusCode::OK);
    let body: BudgetResponseContract = updated.body_as();
    assert_eq!(body.budget.monthly_limit, "275.5000");

    app.put("/api/v1/budget/BILL", Some(&token), json!({ "monthlyLimit": "500" }))
        .await;

    let list: BudgetListResponseContract =
        app.get("/api/v1/budget", Some(&token)).await.body_as();
    let categories: Vec<&str> = list.budget_list.iter().map(|b| b.category.as_str()).collect();
    assert_eq!(categories, ["BILL", "FOOD"], "one row per category, ordered by category");
    assert_eq!(list.budget_list[1].monthly_limit, "275.5000");

    app.cleanup().await;
}

#[tokio::test]
async fn put_budget_rejects_unknown_or_income_category_and_non_positive_limits() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "budget-validation").await;

    for (path, limit) in [
        ("/api/v1/budget/INCOME", "100"),
        ("/api/v1/budget/GADGETS", "100"),
        ("/api/v1/budget/food", "100"),
        ("/api/v1/budget/FOOD", "0"),
        ("/api/v1/budget/FOOD", "-5.00"),
    ] {
        let response = app
            .put(path, Some(&token), json!({ "monthlyLimit": limit }))
            .await;
        assert_eq!(
            response.status,
            StatusCode::BAD_REQUEST,
            "PUT {path} with limit {limit} should be rejected: {:?}",
            response.json
        );
    }

    let list: BudgetListResponseContract =
        app.get("/api/v1/budget", Some(&token)).await.body_as();
    assert!(list.budget_list.is_empty(), "rejected requests must not persist anything");

    app.cleanup().await;
}

#[tokio::test]
async fn delete_budget_returns_204_then_404() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "budget-delete").await;
    app.put("/api/v1/budget/CLOTHING", Some(&token), json!({ "monthlyLimit": "100" }))
        .await;

    let first = app.delete("/api/v1/budget/CLOTHING", Some(&token)).await;
    assert_eq!(first.status, StatusCode::NO_CONTENT);

    let list: BudgetListResponseContract =
        app.get("/api/v1/budget", Some(&token)).await.body_as();
    assert!(list.budget_list.is_empty());

    let second = app.delete("/api/v1/budget/CLOTHING", Some(&token)).await;
    assert_eq!(second.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn delete_budget_rejects_unknown_or_income_category() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "budget-delete-validation").await;

    assert_eq!(
        app.delete("/api/v1/budget/INCOME", Some(&token)).await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.delete("/api/v1/budget/GADGETS", Some(&token)).await.status,
        StatusCode::BAD_REQUEST
    );

    app.cleanup().await;
}

#[tokio::test]
async fn budgets_are_scoped_to_the_caller() {
    let app = TestApp::spawn().await;
    let alice = register_user(&app, "budget-alice").await;
    let bob = register_user(&app, "budget-bob").await;

    app.put("/api/v1/budget/FOOD", Some(&alice), json!({ "monthlyLimit": "300" }))
        .await;

    let bob_list: BudgetListResponseContract =
        app.get("/api/v1/budget", Some(&bob)).await.body_as();
    assert!(bob_list.budget_list.is_empty(), "bob must not see alice's budget");

    // Bob's delete can't reach alice's row, and his own upsert creates a separate row.
    assert_eq!(
        app.delete("/api/v1/budget/FOOD", Some(&bob)).await.status,
        StatusCode::NOT_FOUND
    );
    app.put("/api/v1/budget/FOOD", Some(&bob), json!({ "monthlyLimit": "50" }))
        .await;

    let alice_list: BudgetListResponseContract =
        app.get("/api/v1/budget", Some(&alice)).await.body_as();
    assert_eq!(alice_list.budget_list.len(), 1);
    assert_eq!(alice_list.budget_list[0].monthly_limit, "300.0000");

    app.cleanup().await;
}

#[tokio::test]
async fn budget_routes_require_authentication() {
    let app = TestApp::spawn().await;

    assert_eq!(app.get("/api/v1/budget", None).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        app.put("/api/v1/budget/FOOD", None, json!({ "monthlyLimit": "1" }))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.delete("/api/v1/budget/FOOD", None).await.status,
        StatusCode::UNAUTHORIZED
    );

    app.cleanup().await;
}
