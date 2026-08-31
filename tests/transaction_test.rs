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
    #[serde(rename = "totalElements")]
    total_elements: i64,
    #[serde(rename = "totalPages")]
    total_pages: i64,
}

async fn create_account(app: &TestApp, token: &str, name: &str) -> i64 {
    let response = app
        .post("/api/v1/account", Some(token), json!({ "name": name }))
        .await;
    assert_eq!(response.status, StatusCode::OK, "account setup failed: {:?}", response.json);
    response.json["account"]["id"].as_i64().unwrap()
}

async fn create_transaction(
    app: &TestApp,
    token: &str,
    account_id: i64,
    title: &str,
    amount: &str,
    date: &str,
) {
    let response = app
        .post(
            "/api/v1/transaction",
            Some(token),
            json!({
                "category": "FOOD", "title": title, "amount": amount,
                "date": date, "account": { "id": account_id },
            }),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "transaction setup failed: {:?}", response.json);
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
    assert_eq!(body.total_elements, 2);
    assert_eq!(body.total_pages, 1);

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
    assert_eq!(body.total_elements, 1);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_filters_by_date_range() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-date-range").await;
    let account = create_account(&app, &token, "Checking").await;

    create_transaction(&app, &token, account, "February", "10.00", "2026-02-15").await;
    create_transaction(&app, &token, account, "March", "20.00", "2026-03-15").await;
    create_transaction(&app, &token, account, "April", "30.00", "2026-04-15").await;

    let response = app
        .get(
            "/api/v1/transaction?from=2026-03-01&to=2026-03-31",
            Some(&token),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionListResponseContract = response.body_as();
    assert_eq!(body.total_elements, 1);
    assert_eq!(body.transaction_list[0].title, "March");

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_rejects_from_after_to() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-bad-range").await;

    let response = app
        .get(
            "/api/v1/transaction?from=2026-03-31&to=2026-03-01",
            Some(&token),
        )
        .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_paginates_by_row_count() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-page").await;
    let account = create_account(&app, &token, "Checking").await;

    create_transaction(&app, &token, account, "One", "1.00", "2026-03-01").await;
    create_transaction(&app, &token, account, "Two", "2.00", "2026-03-02").await;
    create_transaction(&app, &token, account, "Three", "3.00", "2026-03-03").await;

    let first_page = app
        .get("/api/v1/transaction?page=0&size=2&sort=date,asc", Some(&token))
        .await;
    let first_page_body: TransactionListResponseContract = first_page.body_as();
    assert_eq!(first_page_body.transaction_list.len(), 2);
    assert_eq!(first_page_body.total_elements, 3);
    assert_eq!(first_page_body.total_pages, 2);
    assert_eq!(first_page_body.transaction_list[0].title, "One");
    assert_eq!(first_page_body.transaction_list[1].title, "Two");

    let second_page = app
        .get("/api/v1/transaction?page=1&size=2&sort=date,asc", Some(&token))
        .await;
    let second_page_body: TransactionListResponseContract = second_page.body_as();
    assert_eq!(second_page_body.transaction_list.len(), 1);
    assert_eq!(second_page_body.transaction_list[0].title, "Three");

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_rejects_oversized_page_size() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-big-page").await;

    let response = app.get("/api/v1/transaction?size=201", Some(&token)).await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_sorts_by_the_requested_field_and_direction() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-sort").await;
    let account = create_account(&app, &token, "Checking").await;

    create_transaction(&app, &token, account, "Cheap", "5.00", "2026-03-01").await;
    create_transaction(&app, &token, account, "Pricey", "50.00", "2026-03-02").await;

    let response = app
        .get("/api/v1/transaction?sort=amount,desc", Some(&token))
        .await;
    let body: TransactionListResponseContract = response.body_as();

    assert_eq!(body.transaction_list[0].title, "Pricey");
    assert_eq!(body.transaction_list[1].title, "Cheap");

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_rejects_an_unknown_sort_field() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-bad-sort").await;

    let response = app
        .get("/api/v1/transaction?sort=bogus,asc", Some(&token))
        .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_filters_by_category() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-category-filter").await;
    let account = create_account(&app, &token, "Checking").await;

    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "FOOD", "title": "Groceries", "amount": "10.00",
            "date": "2026-03-01", "account": { "id": account },
        }),
    )
    .await;
    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "BILL", "title": "Electricity", "amount": "20.00",
            "date": "2026-03-02", "account": { "id": account },
        }),
    )
    .await;

    let response = app.get("/api/v1/transaction?category=BILL", Some(&token)).await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionListResponseContract = response.body_as();
    assert_eq!(body.total_elements, 1);
    assert_eq!(body.total_pages, 1);
    assert_eq!(body.transaction_list.len(), 1);
    assert_eq!(body.transaction_list[0].title, "Electricity");

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_filters_by_account_id() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-account-filter").await;
    let checking = create_account(&app, &token, "Checking").await;
    let savings = create_account(&app, &token, "Savings").await;

    create_transaction(&app, &token, checking, "Checking tx", "10.00", "2026-03-01").await;
    create_transaction(&app, &token, savings, "Savings tx", "20.00", "2026-03-02").await;

    let response = app
        .get(&format!("/api/v1/transaction?account_id={savings}"), Some(&token))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionListResponseContract = response.body_as();
    assert_eq!(body.total_elements, 1);
    assert_eq!(body.total_pages, 1);
    assert_eq!(body.transaction_list.len(), 1);
    assert_eq!(body.transaction_list[0].title, "Savings tx");

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_filters_by_category_and_account_id_together() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-both-filters").await;
    let checking = create_account(&app, &token, "Checking").await;
    let savings = create_account(&app, &token, "Savings").await;

    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "FOOD", "title": "Checking food", "amount": "10.00",
            "date": "2026-03-01", "account": { "id": checking },
        }),
    )
    .await;
    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "BILL", "title": "Checking bill", "amount": "20.00",
            "date": "2026-03-02", "account": { "id": checking },
        }),
    )
    .await;
    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "FOOD", "title": "Savings food", "amount": "30.00",
            "date": "2026-03-03", "account": { "id": savings },
        }),
    )
    .await;

    let response = app
        .get(&format!("/api/v1/transaction?category=FOOD&account_id={checking}"), Some(&token))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionListResponseContract = response.body_as();
    assert_eq!(body.total_elements, 1);
    assert_eq!(body.transaction_list.len(), 1);
    assert_eq!(body.transaction_list[0].title, "Checking food");

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_without_category_or_account_filters_is_unaffected() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-no-filters").await;
    let account = create_account(&app, &token, "Checking").await;

    create_transaction(&app, &token, account, "One", "1.00", "2026-03-01").await;
    create_transaction(&app, &token, account, "Two", "2.00", "2026-03-02").await;

    let response = app.get("/api/v1/transaction", Some(&token)).await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionListResponseContract = response.body_as();
    assert_eq!(body.total_elements, 2);
    assert_eq!(body.transaction_list.len(), 2);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_filtering_by_another_users_account_returns_403() {
    let app = TestApp::spawn().await;
    let alice = register_user(&app, "tx-list-forbidden-alice").await;
    let bob = register_user(&app, "tx-list-forbidden-bob").await;
    let alice_account = create_account(&app, &alice, "Alice Checking").await;

    let response = app
        .get(&format!("/api/v1/transaction?account_id={alice_account}"), Some(&bob))
        .await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_filtering_by_a_nonexistent_account_returns_404() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-list-no-account").await;

    let response = app.get("/api/v1/transaction?account_id=999999", Some(&token)).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_filtered_pagination_reflects_the_filtered_count_across_both_pages() {
    // 5 FOOD transactions (spanning 3 pages at size=2) plus 2 BILL transactions the filter
    // should exclude entirely. If `totalElements`/`totalPages` were computed off the
    // unfiltered total (7) rather than the filtered count (5), or if the two queries drifted
    // out of sync, this would show up as a mismatch between what's counted and what's
    // actually paged through.
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-filtered-paging").await;
    let account = create_account(&app, &token, "Checking").await;

    for (title, date) in [
        ("Food One", "2026-03-01"),
        ("Food Two", "2026-03-02"),
        ("Food Three", "2026-03-03"),
        ("Food Four", "2026-03-04"),
        ("Food Five", "2026-03-05"),
    ] {
        create_transaction(&app, &token, account, title, "1.00", date).await;
    }
    for (title, date) in [("Bill One", "2026-03-06"), ("Bill Two", "2026-03-07")] {
        app.post(
            "/api/v1/transaction",
            Some(&token),
            json!({
                "category": "BILL", "title": title, "amount": "2.00",
                "date": date, "account": { "id": account },
            }),
        )
        .await;
    }

    let mut seen_titles: Vec<String> = Vec::new();
    for page in 0..3 {
        let response = app
            .get(
                &format!("/api/v1/transaction?category=FOOD&page={page}&size=2&sort=date,asc"),
                Some(&token),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK, "page {page}: {:?}", response.json);
        let body: TransactionListResponseContract = response.body_as();
        // Filtered count (5 FOOD), not the unfiltered total (7).
        assert_eq!(body.total_elements, 5, "page {page}");
        assert_eq!(body.total_pages, 3, "page {page}");
        for tx in &body.transaction_list {
            assert!(tx.title.starts_with("Food"), "page {page} leaked a non-FOOD row: {}", tx.title);
        }
        seen_titles.extend(body.transaction_list.iter().map(|t| t.title.clone()));
    }

    seen_titles.sort();
    assert_eq!(
        seen_titles,
        vec!["Food Five", "Food Four", "Food One", "Food Three", "Food Two"],
        "paging through all filtered pages should yield the complete, non-overlapping FOOD set"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_rejects_an_invalid_category_value_with_400() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-bad-category").await;

    let response = app.get("/api/v1/transaction?category=NOTACATEGORY", Some(&token)).await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST, "body: {:?}", response.json);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_rejects_an_empty_category_value_with_400() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-empty-category").await;

    let response = app.get("/api/v1/transaction?category=", Some(&token)).await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST, "body: {:?}", response.json);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_date_range_category_and_account_combine_with_and_semantics() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-triple-filter").await;
    let checking = create_account(&app, &token, "Checking").await;
    let savings = create_account(&app, &token, "Savings").await;

    // Matches all three filters (in range, FOOD, checking).
    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "FOOD", "title": "Match", "amount": "10.00",
            "date": "2026-03-15", "account": { "id": checking },
        }),
    )
    .await;
    // In range + checking, but wrong category — should be excluded by the category filter.
    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "BILL", "title": "Wrong category", "amount": "10.00",
            "date": "2026-03-16", "account": { "id": checking },
        }),
    )
    .await;
    // In range + FOOD, but wrong account — should be excluded by the account filter.
    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "FOOD", "title": "Wrong account", "amount": "10.00",
            "date": "2026-03-17", "account": { "id": savings },
        }),
    )
    .await;
    // FOOD + checking, but outside the date range — should be excluded by the date filter.
    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "FOOD", "title": "Wrong date", "amount": "10.00",
            "date": "2026-04-01", "account": { "id": checking },
        }),
    )
    .await;

    let response = app
        .get(
            &format!(
                "/api/v1/transaction?from=2026-03-01&to=2026-03-31&category=FOOD&account_id={checking}"
            ),
            Some(&token),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionListResponseContract = response.body_as();
    assert_eq!(body.total_elements, 1, "body: {:?}", body.transaction_list.iter().map(|t| &t.title).collect::<Vec<_>>());
    assert_eq!(body.transaction_list.len(), 1);
    assert_eq!(body.transaction_list[0].title, "Match");

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_category_and_account_with_zero_matches_returns_empty_not_error() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-zero-matches").await;
    let checking = create_account(&app, &token, "Checking").await;
    let savings = create_account(&app, &token, "Savings").await;

    // FOOD exists, but only on `savings`; BILL exists, but only on `checking`. Filtering for
    // FOOD on `checking` should therefore match nothing.
    create_transaction(&app, &token, savings, "Savings food", "5.00", "2026-03-01").await;
    app.post(
        "/api/v1/transaction",
        Some(&token),
        json!({
            "category": "BILL", "title": "Checking bill", "amount": "5.00",
            "date": "2026-03-02", "account": { "id": checking },
        }),
    )
    .await;

    let response = app
        .get(&format!("/api/v1/transaction?category=FOOD&account_id={checking}"), Some(&token))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionListResponseContract = response.body_as();
    assert_eq!(body.total_elements, 0);
    assert_eq!(body.total_pages, 0);
    assert!(body.transaction_list.is_empty());

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_filtering_by_own_account_id_works_end_to_end() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-own-account-filter").await;
    let checking = create_account(&app, &token, "Checking").await;
    let savings = create_account(&app, &token, "Savings").await;

    create_transaction(&app, &token, checking, "Checking A", "1.00", "2026-03-01").await;
    create_transaction(&app, &token, checking, "Checking B", "2.00", "2026-03-02").await;
    create_transaction(&app, &token, savings, "Savings A", "3.00", "2026-03-03").await;

    let response = app
        .get(&format!("/api/v1/transaction?account_id={checking}"), Some(&token))
        .await;

    assert_eq!(response.status, StatusCode::OK, "body: {:?}", response.json);
    let body: TransactionListResponseContract = response.body_as();
    assert_eq!(body.total_elements, 2);
    let mut titles: Vec<&str> = body.transaction_list.iter().map(|t| t.title.as_str()).collect();
    titles.sort();
    assert_eq!(titles, vec!["Checking A", "Checking B"]);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_rejects_a_non_numeric_account_id_with_400() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-nonnumeric-account").await;

    let response = app.get("/api/v1/transaction?account_id=abc", Some(&token)).await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST, "body: {:?}", response.json);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_negative_account_id_returns_404_not_a_server_error() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-negative-account").await;

    let response = app.get("/api/v1/transaction?account_id=-1", Some(&token)).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND, "body: {:?}", response.json);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_zero_account_id_returns_404_not_a_server_error() {
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-zero-account").await;

    let response = app.get("/api/v1/transaction?account_id=0", Some(&token)).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND, "body: {:?}", response.json);

    app.cleanup().await;
}

#[tokio::test]
async fn transaction_list_unfiltered_returns_every_transaction_across_categories_and_accounts() {
    // Regression check for the plain (no `category`/`account_id`) path with a larger, more
    // varied dataset than the existing minimal coverage: many transactions, several accounts,
    // every category, still all returned unfiltered.
    let app = TestApp::spawn().await;
    let token = register_user(&app, "tx-large-unfiltered").await;
    let checking = create_account(&app, &token, "Checking").await;
    let savings = create_account(&app, &token, "Savings").await;

    let categories = ["FOOD", "BILL", "SUBSCRIPTION", "CLOTHING", "INCOME"];
    let mut expected_titles: Vec<String> = Vec::new();
    for (i, category) in categories.iter().enumerate() {
        for j in 0..3 {
            let account = if (i + j) % 2 == 0 { checking } else { savings };
            let title = format!("{category}-{j}");
            app.post(
                "/api/v1/transaction",
                Some(&token),
                json!({
                    "category": category, "title": title, "amount": "1.00",
                    "date": format!("2026-03-{:02}", i * 3 + j + 1), "account": { "id": account },
                }),
            )
            .await;
            expected_titles.push(title);
        }
    }

    let response = app.get("/api/v1/transaction?size=200", Some(&token)).await;

    assert_eq!(response.status, StatusCode::OK);
    let body: TransactionListResponseContract = response.body_as();
    assert_eq!(body.total_elements, expected_titles.len() as i64);
    assert_eq!(body.transaction_list.len(), expected_titles.len());
    let mut actual_titles: Vec<String> = body.transaction_list.iter().map(|t| t.title.clone()).collect();
    actual_titles.sort();
    expected_titles.sort();
    assert_eq!(actual_titles, expected_titles);

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
