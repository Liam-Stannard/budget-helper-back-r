//! Contract tests for `/api/v1/account*`. See docs/api-spec.md.
//!
//! `create`/`update` used to return an empty `200 OK` body while the frontend expected
//! `{ "account": {...} }` back — the same class of bug as the transaction list crash, just
//! quieter (an Angular JSON-parse failure instead of a null-deref). The wrapped-response
//! assertions below are what would have caught that.

mod common;

use axum::http::StatusCode;
use common::{register_user, TestApp};
use serde::Deserialize;
use serde_json::json;

// Deliberately doesn't deny unknown fields: the wire object also carries `user_id`, which
// `IAccount` on the frontend doesn't declare but tolerates as an extra field. What matters is
// that `id`/`name`/`startingBalance`/`currentBalance` are present — that's the part a client
// actually depends on.
#[derive(Deserialize)]
struct AccountContract {
    id: i64,
    name: String,
    #[serde(rename = "startingBalance")]
    starting_balance: String,
    #[serde(rename = "currentBalance")]
    current_balance: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountResponseContract {
    account: AccountContract,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountListResponseContract {
    #[serde(rename = "accountList")]
    account_list: Vec<AccountContract>,
}

#[tokio::test]
async fn create_account_returns_the_wrapped_created_account() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "acc-create").await;

    let response = app
        .post("/api/v1/account", Some(&token), json!({ "name": "Checking" }))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: AccountResponseContract = response.body_as();
    assert_eq!(body.account.name, "Checking");
    assert!(body.account.id > 0);
    // No startingBalance was sent, so both default to zero.
    assert_eq!(body.account.starting_balance, "0.0000");
    assert_eq!(body.account.current_balance, "0.0000");

    app.cleanup().await;
}

#[tokio::test]
async fn create_account_with_a_starting_balance_returns_it_as_the_current_balance() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "acc-starting-balance").await;

    let response = app
        .post(
            "/api/v1/account",
            Some(&token),
            json!({ "name": "Checking", "startingBalance": "500.00" }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: AccountResponseContract = response.body_as();
    assert_eq!(body.account.starting_balance, "500.0000");
    // No transactions yet, so current == starting.
    assert_eq!(body.account.current_balance, "500.0000");

    app.cleanup().await;
}

#[tokio::test]
async fn current_balance_reflects_income_and_expense_transactions() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "acc-balance-tx").await;
    let created = app
        .post("/api/v1/account", Some(&token), json!({ "name": "Checking" }))
        .await;
    let account_id = created.json["account"]["id"].as_i64().unwrap();

    app.put(
        &format!("/api/v1/account/id/{account_id}"),
        Some(&token),
        json!({ "name": "Checking", "startingBalance": "1000.00" }),
    )
    .await;

    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "INCOME", "title": "Paycheck", "amount": "300.00",
            "date": "2026-03-01", "account": { "id": account_id },
        }),
    )
    .await;
    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "FOOD", "title": "Groceries", "amount": "50.00",
            "date": "2026-03-02", "account": { "id": account_id },
        }),
    )
    .await;

    let response = app
        .get(&format!("/api/v1/account/id/{account_id}"), Some(&token))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: AccountResponseContract = response.body_as();
    // 1000 starting + 300 income - 50 expense
    assert_eq!(body.account.current_balance, "1250.0000");

    app.cleanup().await;
}

#[tokio::test]
async fn current_balance_moves_with_a_transaction_reassigned_to_another_account() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "acc-balance-reassign").await;
    let created_a = app
        .post("/api/v1/account", Some(&token), json!({ "name": "Account A" }))
        .await;
    let account_a = created_a.json["account"]["id"].as_i64().unwrap();
    let created_b = app
        .post("/api/v1/account", Some(&token), json!({ "name": "Account B" }))
        .await;
    let account_b = created_b.json["account"]["id"].as_i64().unwrap();

    let created = app
        .post(
            "/api/v1/transaction",
            Some(&token),
            json!({
                "category": "FOOD", "title": "Groceries", "amount": "40.00",
                "date": "2026-03-01", "account": { "id": account_a },
            }),
        )
        .await;
    let transaction_id = created.json["transaction"]["id"].as_i64().unwrap();

    let before_a: AccountResponseContract = app
        .get(&format!("/api/v1/account/id/{account_a}"), Some(&token))
        .await
        .body_as();
    let before_b: AccountResponseContract = app
        .get(&format!("/api/v1/account/id/{account_b}"), Some(&token))
        .await
        .body_as();
    assert_eq!(before_a.account.current_balance, "-40.0000");
    assert_eq!(before_b.account.current_balance, "0.0000");

    app.put(
        &format!("/api/v1/transaction/id/{transaction_id}"),
        Some(&token),
        json!({
            "category": "FOOD", "title": "Groceries", "amount": "40.00",
            "date": "2026-03-01", "account": { "id": account_b },
        }),
    )
    .await;

    let after_a: AccountResponseContract = app
        .get(&format!("/api/v1/account/id/{account_a}"), Some(&token))
        .await
        .body_as();
    let after_b: AccountResponseContract = app
        .get(&format!("/api/v1/account/id/{account_b}"), Some(&token))
        .await
        .body_as();
    assert_eq!(
        after_a.account.current_balance, "0.0000",
        "balance should follow the transaction, not stay stuck on the old account"
    );
    assert_eq!(after_b.account.current_balance, "-40.0000");

    app.cleanup().await;
}

#[tokio::test]
async fn list_accounts_returns_only_the_callers_accounts() {
    let app = TestApp::spawn().await;
    let alice = register_user(&app, "acc-list-alice").await;
    let bob = register_user(&app, "acc-list-bob").await;

    app.post("/api/v1/account", Some(&alice), json!({ "name": "Alice Checking" }))
        .await;
    app.post("/api/v1/account", Some(&alice), json!({ "name": "Alice Savings" }))
        .await;
    app.post("/api/v1/account", Some(&bob), json!({ "name": "Bob Checking" }))
        .await;

    let response = app.get("/api/v1/account", Some(&alice)).await;
    assert_eq!(response.status, StatusCode::OK);
    let body: AccountListResponseContract = response.body_as();

    let names: Vec<&str> = body.account_list.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names.len(), 2, "should not see bob's account: {names:?}");
    assert!(names.contains(&"Alice Checking"));
    assert!(names.contains(&"Alice Savings"));

    app.cleanup().await;
}

#[tokio::test]
async fn get_account_by_id_returns_the_wrapped_account() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "acc-get").await;
    let created = app
        .post("/api/v1/account", Some(&token), json!({ "name": "Savings" }))
        .await;
    let id = created.json["account"]["id"].as_i64().unwrap();

    let response = app.get(&format!("/api/v1/account/id/{id}"), Some(&token)).await;

    assert_eq!(response.status, StatusCode::OK);
    let body: AccountResponseContract = response.body_as();
    assert_eq!(body.account.id, id);
    assert_eq!(body.account.name, "Savings");

    app.cleanup().await;
}

#[tokio::test]
async fn get_nonexistent_account_returns_404() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "acc-get-404").await;

    let response = app.get("/api/v1/account/id/999999", Some(&token)).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn get_another_users_account_returns_404_not_403() {
    let app = TestApp::spawn().await;
    let alice = register_user(&app, "acc-cross-alice").await;
    let bob = register_user(&app, "acc-cross-bob").await;

    let created = app
        .post("/api/v1/account", Some(&alice), json!({ "name": "Alice Only" }))
        .await;
    let id = created.json["account"]["id"].as_i64().unwrap();

    let response = app.get(&format!("/api/v1/account/id/{id}"), Some(&bob)).await;

    assert_eq!(
        response.status,
        StatusCode::NOT_FOUND,
        "cross-user access should look like the resource doesn't exist, not 403"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn update_account_returns_the_wrapped_updated_account_and_persists() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "acc-update").await;
    let created = app
        .post("/api/v1/account", Some(&token), json!({ "name": "Old Name" }))
        .await;
    let id = created.json["account"]["id"].as_i64().unwrap();

    let response = app
        .put(
            &format!("/api/v1/account/id/{id}"),
            Some(&token),
            json!({ "name": "New Name" }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: AccountResponseContract = response.body_as();
    assert_eq!(body.account.name, "New Name");

    let refetched = app.get(&format!("/api/v1/account/id/{id}"), Some(&token)).await;
    let refetched_body: AccountResponseContract = refetched.body_as();
    assert_eq!(refetched_body.account.name, "New Name");

    app.cleanup().await;
}

#[tokio::test]
async fn update_another_users_account_returns_404() {
    let app = TestApp::spawn().await;
    let alice = register_user(&app, "acc-update-cross-alice").await;
    let bob = register_user(&app, "acc-update-cross-bob").await;

    let created = app
        .post("/api/v1/account", Some(&alice), json!({ "name": "Alice's" }))
        .await;
    let id = created.json["account"]["id"].as_i64().unwrap();

    let response = app
        .put(
            &format!("/api/v1/account/id/{id}"),
            Some(&bob),
            json!({ "name": "Hijacked" }),
        )
        .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn delete_account_succeeds_and_then_get_returns_404() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "acc-delete").await;
    let created = app
        .post("/api/v1/account", Some(&token), json!({ "name": "Throwaway" }))
        .await;
    let id = created.json["account"]["id"].as_i64().unwrap();

    let delete_response = app.delete(&format!("/api/v1/account/id/{id}"), Some(&token)).await;
    assert_eq!(delete_response.status, StatusCode::OK);

    let get_response = app.get(&format!("/api/v1/account/id/{id}"), Some(&token)).await;
    assert_eq!(get_response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn delete_nonexistent_account_returns_404() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "acc-delete-404").await;

    let response = app.delete("/api/v1/account/id/999999", Some(&token)).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn delete_account_with_transactions_returns_409() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "acc-delete-conflict").await;
    let created = app
        .post("/api/v1/account", Some(&token), json!({ "name": "Has Transactions" }))
        .await;
    let account_id = created.json["account"]["id"].as_i64().unwrap();

    let tx = app
        .post(
            "/api/v1/transaction",
            Some(&token),
            json!({
                "category": "FOOD",
                "title": "Groceries",
                "amount": "12.50",
                "date": "2026-01-01",
                "account": { "id": account_id },
            }),
        )
        .await;
    assert_eq!(tx.status, StatusCode::OK, "transaction setup failed: {:?}", tx.json);

    let response = app
        .delete(&format!("/api/v1/account/id/{account_id}"), Some(&token))
        .await;

    assert_eq!(response.status, StatusCode::CONFLICT);

    app.cleanup().await;
}

#[tokio::test]
async fn account_routes_require_auth() {
    let app = TestApp::spawn().await;

    assert_eq!(app.get("/api/v1/account", None).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        app.post("/api/v1/account", None, json!({ "name": "x" })).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get("/api/v1/account/id/1", None).await.status,
        StatusCode::UNAUTHORIZED
    );

    app.cleanup().await;
}
