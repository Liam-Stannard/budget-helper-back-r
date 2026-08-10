//! Contract tests for `/api/v1/transaction*`. See docs/api-spec.md.
//!
//! This is the domain the reported bug lived in: the transaction list/get responses used to
//! serialize a flat `account_id` instead of a nested `account` object, so the Angular list
//! screen crashed on `element.account.name` being read off `undefined`. The
//! `TransactionContract` below requires a nested `account: { id, name }` — deserializing into
//! it is exactly the check that would have caught the original bug (it fails with "missing
//! field `account`" against the old, broken response shape).

mod common;

use axum::http::StatusCode;
use common::{register_user, TestApp};
use serde::Deserialize;
use serde_json::json;

// Not `deny_unknown_fields`: the account object also carries `user_id`, same as in
// account_test.rs. `id`/`name` being required is what matters here.
#[derive(Deserialize)]
struct AccountRefContract {
    id: i64,
    name: String,
}

#[derive(Deserialize)]
struct TransactionContract {
    id: i64,
    category: String,
    title: String,
    amount: String,
    date: String,
    account: AccountRefContract,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionResponseContract {
    transaction: TransactionContract,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionListResponseContract {
    #[serde(rename = "transactionList")]
    transaction_list: Vec<TransactionContract>,
}

async fn create_account(app: &TestApp, token: &str, name: &str) -> i64 {
    let response = app
        .post("/api/v1/account", Some(token), json!({ "name": name }))
        .await;
    assert_eq!(response.status, StatusCode::OK, "account setup failed: {:?}", response.json);
    response.json["account"]["id"].as_i64().unwrap()
}

#[tokio::test]
async fn create_transaction_returns_wrapped_transaction_with_real_nested_account() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-create").await;
    let account_id = create_account(&app, &token, "Checking").await;

    let response = app
        .post(
            "/api/v1/transaction",
            Some(&token),
            json!({
                "category": "FOOD",
                "title": "Groceries",
                "amount": "42.10",
                "date": "2026-03-01",
                "account": { "id": account_id },
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionResponseContract = response.body_as();
    assert_eq!(body.transaction.title, "Groceries");
    assert_eq!(body.transaction.category, "FOOD");
    // The `amount` column is DECIMAL(19,4), so the value round-trips at 4 decimal places
    // regardless of how many the client sent — "42.10" in, "42.1000" back out.
    assert_eq!(body.transaction.amount, "42.1000");
    assert_eq!(body.transaction.date, "2026-03-01");
    assert_eq!(body.transaction.account.id, account_id);
    // Proves the account was actually looked up (joined), not just echoed from the request.
    assert_eq!(body.transaction.account.name, "Checking");

    app.cleanup().await;
}

#[tokio::test]
async fn list_transactions_nests_the_account_object_for_every_row() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-list").await;
    let checking = create_account(&app, &token, "Checking").await;
    let savings = create_account(&app, &token, "Savings").await;

    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "BILL", "title": "Electricity", "amount": "80.00",
            "date": "2026-03-02", "account": { "id": checking },
        }),
    )
    .await;
    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "SUBSCRIPTION", "title": "Streaming", "amount": "15.99",
            "date": "2026-03-03", "account": { "id": savings },
        }),
    )
    .await;

    let response = app.get("/api/v1/transaction", Some(&token)).await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionListResponseContract = response.body_as();
    assert_eq!(body.transaction_list.len(), 2);

    let electricity = body
        .transaction_list
        .iter()
        .find(|t| t.title == "Electricity")
        .expect("electricity transaction present");
    assert_eq!(electricity.account.name, "Checking");

    let streaming = body
        .transaction_list
        .iter()
        .find(|t| t.title == "Streaming")
        .expect("streaming transaction present");
    assert_eq!(streaming.account.name, "Savings");

    app.cleanup().await;
}

#[tokio::test]
async fn get_transaction_by_id_nests_the_account_object() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-get").await;
    let account_id = create_account(&app, &token, "Wallet").await;
    let created = app
        .post(
            "/api/v1/transaction",
            Some(&token),
            json!({
                "category": "CLOTHING", "title": "Jacket", "amount": "89.99",
                "date": "2026-03-04", "account": { "id": account_id },
            }),
        )
        .await;
    let id = created.json["transaction"]["id"].as_i64().unwrap();

    let response = app.get(&format!("/api/v1/transaction/id/{id}"), Some(&token)).await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionResponseContract = response.body_as();
    assert_eq!(body.transaction.id, id);
    assert_eq!(body.transaction.account.name, "Wallet");

    app.cleanup().await;
}

#[tokio::test]
async fn get_nonexistent_transaction_returns_404() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-get-404").await;

    let response = app.get("/api/v1/transaction/id/999999", Some(&token)).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn create_transaction_against_another_users_account_returns_403() {
    let app = TestApp::spawn().await;
    let alice = register_user(&app, "tx-forbidden-alice").await;
    let bob = register_user(&app, "tx-forbidden-bob").await;
    let alice_account = create_account(&app, &alice, "Alice Checking").await;

    let response = app
        .post(
            "/api/v1/transaction",
            Some(&bob),
            json!({
                "category": "FOOD", "title": "Sneaky", "amount": "1.00",
                "date": "2026-03-05", "account": { "id": alice_account },
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);

    app.cleanup().await;
}

#[tokio::test]
async fn create_transaction_against_a_nonexistent_account_returns_404() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-no-account").await;

    let response = app
        .post(
            "/api/v1/transaction",
            Some(&token),
            json!({
                "category": "FOOD", "title": "Ghost", "amount": "1.00",
                "date": "2026-03-05", "account": { "id": 999999 },
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn update_transaction_returns_wrapped_transaction_and_persists_changes() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-update").await;
    let account_id = create_account(&app, &token, "Checking").await;
    let created = app
        .post(
            "/api/v1/transaction",
            Some(&token),
            json!({
                "category": "FOOD", "title": "Lunch", "amount": "10.00",
                "date": "2026-03-06", "account": { "id": account_id },
            }),
        )
        .await;
    let id = created.json["transaction"]["id"].as_i64().unwrap();

    let response = app
        .put(
            &format!("/api/v1/transaction/id/{id}"),
            Some(&token),
            json!({
                "category": "BILL", "title": "Lunch (corrected)", "amount": "11.50",
                "date": "2026-03-07", "account": { "id": account_id },
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionResponseContract = response.body_as();
    assert_eq!(body.transaction.title, "Lunch (corrected)");
    assert_eq!(body.transaction.category, "BILL");
    assert_eq!(body.transaction.amount, "11.5000"); // DECIMAL(19,4) — see comment above

    let refetched = app.get(&format!("/api/v1/transaction/id/{id}"), Some(&token)).await;
    let refetched_body: TransactionResponseContract = refetched.body_as();
    assert_eq!(refetched_body.transaction.title, "Lunch (corrected)");

    app.cleanup().await;
}

#[tokio::test]
async fn update_transaction_can_reassign_to_another_owned_account() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-reassign").await;
    let checking = create_account(&app, &token, "Checking").await;
    let savings = create_account(&app, &token, "Savings").await;
    let created = app
        .post(
            "/api/v1/transaction",
            Some(&token),
            json!({
                "category": "FOOD", "title": "Move me", "amount": "5.00",
                "date": "2026-03-08", "account": { "id": checking },
            }),
        )
        .await;
    let id = created.json["transaction"]["id"].as_i64().unwrap();

    let response = app
        .put(
            &format!("/api/v1/transaction/id/{id}"),
            Some(&token),
            json!({
                "category": "FOOD", "title": "Move me", "amount": "5.00",
                "date": "2026-03-08", "account": { "id": savings },
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionResponseContract = response.body_as();
    assert_eq!(body.transaction.account.id, savings);
    assert_eq!(body.transaction.account.name, "Savings");

    app.cleanup().await;
}

#[tokio::test]
async fn update_transaction_against_another_users_account_returns_403() {
    let app = TestApp::spawn().await;
    let alice = register_user(&app, "tx-update-forbidden-alice").await;
    let bob = register_user(&app, "tx-update-forbidden-bob").await;
    let alice_account = create_account(&app, &alice, "Alice Checking").await;
    let bob_account = create_account(&app, &bob, "Bob Checking").await;
    let created = app
        .post(
            "/api/v1/transaction",
            Some(&bob),
            json!({
                "category": "FOOD", "title": "Bob's", "amount": "5.00",
                "date": "2026-03-09", "account": { "id": bob_account },
            }),
        )
        .await;
    let id = created.json["transaction"]["id"].as_i64().unwrap();

    let response = app
        .put(
            &format!("/api/v1/transaction/id/{id}"),
            Some(&bob),
            json!({
                "category": "FOOD", "title": "Bob's", "amount": "5.00",
                "date": "2026-03-09", "account": { "id": alice_account },
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);

    app.cleanup().await;
}

#[tokio::test]
async fn update_another_users_transaction_returns_404() {
    // Uses bob's own account in the payload so this isolates the transaction-ownership check
    // (404) from the separate account-ownership check (403, covered by
    // `update_transaction_against_another_users_account_returns_403` above) — the handler
    // checks account ownership first, so reusing alice's account here would 403 instead.
    let app = TestApp::spawn().await;
    let alice = register_user(&app, "tx-cross-update-alice").await;
    let bob = register_user(&app, "tx-cross-update-bob").await;
    let alice_account = create_account(&app, &alice, "Alice Checking").await;
    let bob_account = create_account(&app, &bob, "Bob Checking").await;
    let created = app
        .post(
            "/api/v1/transaction",
            Some(&alice),
            json!({
                "category": "FOOD", "title": "Alice's", "amount": "5.00",
                "date": "2026-03-10", "account": { "id": alice_account },
            }),
        )
        .await;
    let id = created.json["transaction"]["id"].as_i64().unwrap();

    let response = app
        .put(
            &format!("/api/v1/transaction/id/{id}"),
            Some(&bob),
            json!({
                "category": "FOOD", "title": "Hijacked", "amount": "5.00",
                "date": "2026-03-10", "account": { "id": bob_account },
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn delete_transaction_succeeds_and_then_get_returns_404() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-delete").await;
    let account_id = create_account(&app, &token, "Checking").await;
    let created = app
        .post(
            "/api/v1/transaction",
            Some(&token),
            json!({
                "category": "FOOD", "title": "Throwaway", "amount": "1.00",
                "date": "2026-03-11", "account": { "id": account_id },
            }),
        )
        .await;
    let id = created.json["transaction"]["id"].as_i64().unwrap();

    let delete_response = app.delete(&format!("/api/v1/transaction/id/{id}"), Some(&token)).await;
    assert_eq!(delete_response.status, StatusCode::OK);

    let get_response = app.get(&format!("/api/v1/transaction/id/{id}"), Some(&token)).await;
    assert_eq!(get_response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn delete_another_users_transaction_returns_404() {
    let app = TestApp::spawn().await;
    let alice = register_user(&app, "tx-cross-delete-alice").await;
    let bob = register_user(&app, "tx-cross-delete-bob").await;
    let alice_account = create_account(&app, &alice, "Alice Checking").await;
    let created = app
        .post(
            "/api/v1/transaction",
            Some(&alice),
            json!({
                "category": "FOOD", "title": "Alice's", "amount": "5.00",
                "date": "2026-03-12", "account": { "id": alice_account },
            }),
        )
        .await;
    let id = created.json["transaction"]["id"].as_i64().unwrap();

    let response = app.delete(&format!("/api/v1/transaction/id/{id}"), Some(&bob)).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_only_includes_the_callers_own_transactions() {
    let app = TestApp::spawn().await;
    let alice = register_user(&app, "tx-isolation-alice").await;
    let bob = register_user(&app, "tx-isolation-bob").await;
    let alice_account = create_account(&app, &alice, "Alice Checking").await;
    let bob_account = create_account(&app, &bob, "Bob Checking").await;

    app.post(
        "/api/v1/transaction",
        Some(&alice),
        json!({
            "category": "FOOD", "title": "Alice's lunch", "amount": "5.00",
            "date": "2026-03-13", "account": { "id": alice_account },
        }),
    )
    .await;
    app.post(
        "/api/v1/transaction",
        Some(&bob),
        json!({
            "category": "FOOD", "title": "Bob's lunch", "amount": "5.00",
            "date": "2026-03-13", "account": { "id": bob_account },
        }),
    )
    .await;

    let response = app.get("/api/v1/transaction", Some(&alice)).await;
    let body: TransactionListResponseContract = response.body_as();

    assert_eq!(body.transaction_list.len(), 1);
    assert_eq!(body.transaction_list[0].title, "Alice's lunch");

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_routes_require_auth() {
    let app = TestApp::spawn().await;

    assert_eq!(app.get("/api/v1/transaction", None).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        app.post(
            "/api/v1/transaction",
            None,
            json!({
                "category": "FOOD", "title": "x", "amount": "1.00",
                "date": "2026-01-01", "account": { "id": 1 },
            }),
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );

    app.cleanup().await;
}
