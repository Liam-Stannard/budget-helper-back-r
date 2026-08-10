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
// that `id`/`name` are present — that's the part a client actually depends on.
#[derive(Deserialize)]
struct AccountContract {
    id: i64,
    name: String,
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
