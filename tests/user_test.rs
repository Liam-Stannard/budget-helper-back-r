//! Contract tests for `/api/v1/users*`. See docs/api-spec.md "Users (admin)" section.
//!
//! These routes are gated to `ROLE_ADMIN` by the `require_admin` middleware (`src/middleware/
//! auth.rs`), layered inside `require_auth` on the `admin_only` sub-router in `src/app.rs`. A
//! plain `ROLE_USER` caller gets 403; an unauthenticated caller still gets 401 (require_auth
//! runs first and never reaches the role check).

mod common;

use axum::http::StatusCode;
use common::{register_admin, register_user, TestApp};
use serde::Deserialize;
use serde_json::json;

// `password` deliberately isn't a field here: if `#[serde(skip_serializing)]` on
// `User::password` were ever removed, this would start failing with "unknown field `password`"
// instead of silently leaking a password hash into API responses.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UserContract {
    id: i64,
    username: String,
    role: String,
    email: String,
}

async fn create_user(app: &TestApp, admin_token: &str, username: &str, email: &str) -> serde_json::Value {
    let response = app
        .post(
            "/api/v1/users",
            Some(admin_token),
            json!({
                "username": username,
                "role": "ROLE_USER",
                "email": email,
                "password": "some-password-123",
            }),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "user setup failed: {:?}", response.json);
    response.json
}

#[tokio::test]
async fn admin_can_list_every_user_without_password_hashes() {
    let app = TestApp::spawn().await;
    let admin = register_admin(&app, "users-list-admin").await;
    let _other = register_user(&app, "users-list-other").await;

    let response = app.get("/api/v1/users", Some(&admin)).await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.json.is_array(), "expected a bare array, got: {}", response.json);
    let users: Vec<UserContract> = response.body_as();
    assert!(users.len() >= 2, "admin should see every user, including non-admins");
    assert!(!response.json.to_string().to_lowercase().contains("password"));

    app.cleanup().await;
}

#[tokio::test]
async fn admin_create_user_returns_the_created_user_without_password() {
    let app = TestApp::spawn().await;
    let admin = register_admin(&app, "users-create-admin").await;

    let created = create_user(&app, &admin, "frank", "frank@example.com").await;
    let contract: UserContract = serde_json::from_value(created.clone())
        .unwrap_or_else(|e| panic!("create response didn't match contract: {e}\nbody: {created}"));

    assert_eq!(contract.username, "frank");
    assert_eq!(contract.role, "ROLE_USER");
    assert_eq!(contract.email, "frank@example.com");

    app.cleanup().await;
}

#[tokio::test]
async fn admin_create_user_with_duplicate_email_returns_409() {
    let app = TestApp::spawn().await;
    let admin = register_admin(&app, "users-dupe-admin").await;
    create_user(&app, &admin, "grace", "grace@example.com").await;

    let response = app
        .post(
            "/api/v1/users",
            Some(&admin),
            json!({
                "username": "grace-2",
                "role": "ROLE_USER",
                "email": "grace@example.com",
                "password": "some-password-123",
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::CONFLICT);

    app.cleanup().await;
}

#[tokio::test]
async fn admin_get_user_by_id_returns_that_user() {
    let app = TestApp::spawn().await;
    let admin = register_admin(&app, "users-get-admin").await;
    let created = create_user(&app, &admin, "henry", "henry@example.com").await;
    let id = created["id"].as_i64().unwrap();

    let response = app.get(&format!("/api/v1/users/{id}"), Some(&admin)).await;

    assert_eq!(response.status, StatusCode::OK);
    let contract: UserContract = response.body_as();
    assert_eq!(contract.id, id);
    assert_eq!(contract.username, "henry");

    app.cleanup().await;
}

#[tokio::test]
async fn admin_get_nonexistent_user_returns_404() {
    let app = TestApp::spawn().await;
    let admin = register_admin(&app, "users-get-404-admin").await;

    let response = app.get("/api/v1/users/999999", Some(&admin)).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn admin_update_user_requires_resending_a_password_and_returns_the_updated_user() {
    let app = TestApp::spawn().await;
    let admin = register_admin(&app, "users-update-admin").await;
    let created = create_user(&app, &admin, "iris", "iris@example.com").await;
    let id = created["id"].as_i64().unwrap();

    let response = app
        .put(
            &format!("/api/v1/users/{id}"),
            Some(&admin),
            json!({
                "username": "iris-renamed",
                "role": "ROLE_USER",
                "email": "iris@example.com",
                "password": "a-new-password-value",
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let contract: UserContract = response.body_as();
    assert_eq!(contract.username, "iris-renamed");

    app.cleanup().await;
}

#[tokio::test]
async fn admin_delete_user_succeeds_and_then_get_returns_404() {
    let app = TestApp::spawn().await;
    let admin = register_admin(&app, "users-delete-admin").await;
    let created = create_user(&app, &admin, "jack", "jack@example.com").await;
    let id = created["id"].as_i64().unwrap();

    let delete_response = app.delete(&format!("/api/v1/users/{id}"), Some(&admin)).await;
    assert_eq!(delete_response.status, StatusCode::OK);

    let get_response = app.get(&format!("/api/v1/users/{id}"), Some(&admin)).await;
    assert_eq!(get_response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn admin_delete_nonexistent_user_returns_404() {
    let app = TestApp::spawn().await;
    let admin = register_admin(&app, "users-delete-404-admin").await;

    let response = app.delete("/api/v1/users/999999", Some(&admin)).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn unauthenticated_request_returns_401_not_403() {
    // require_auth wraps require_admin, so a missing/invalid token should fail at the auth
    // check before the role check is ever reached.
    let app = TestApp::spawn().await;

    assert_eq!(app.get("/api/v1/users", None).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(app.get("/api/v1/users/1", None).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        app.post("/api/v1/users", None, json!({})).await.status,
        StatusCode::UNAUTHORIZED
    );

    app.cleanup().await;
}

#[tokio::test]
async fn non_admin_list_users_returns_403() {
    let app = TestApp::spawn().await;
    let user = register_user(&app, "users-rbac-list").await;

    let response = app.get("/api/v1/users", Some(&user)).await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);

    app.cleanup().await;
}

#[tokio::test]
async fn non_admin_get_user_by_id_returns_403() {
    let app = TestApp::spawn().await;
    let user = register_user(&app, "users-rbac-get").await;

    let response = app.get("/api/v1/users/1", Some(&user)).await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);

    app.cleanup().await;
}

#[tokio::test]
async fn non_admin_create_user_returns_403() {
    let app = TestApp::spawn().await;
    let user = register_user(&app, "users-rbac-create").await;

    let response = app
        .post(
            "/api/v1/users",
            Some(&user),
            json!({
                "username": "should-not-be-created",
                "role": "ROLE_USER",
                "email": "nope@example.com",
                "password": "whatever-123",
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);

    app.cleanup().await;
}

#[tokio::test]
async fn non_admin_update_user_returns_403() {
    let app = TestApp::spawn().await;
    let admin = register_admin(&app, "users-rbac-update-admin").await;
    let user = register_user(&app, "users-rbac-update-user").await;
    let target = create_user(&app, &admin, "kate", "kate@example.com").await;
    let id = target["id"].as_i64().unwrap();

    let response = app
        .put(
            &format!("/api/v1/users/{id}"),
            Some(&user),
            json!({
                "username": "hijacked",
                "role": "ROLE_ADMIN",
                "email": "kate@example.com",
                "password": "whatever-123",
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);

    app.cleanup().await;
}

#[tokio::test]
async fn non_admin_delete_user_returns_403() {
    let app = TestApp::spawn().await;
    let admin = register_admin(&app, "users-rbac-delete-admin").await;
    let user = register_user(&app, "users-rbac-delete-user").await;
    let target = create_user(&app, &admin, "leo", "leo@example.com").await;
    let id = target["id"].as_i64().unwrap();

    let response = app.delete(&format!("/api/v1/users/{id}"), Some(&user)).await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);

    // Confirm it really wasn't deleted.
    let still_there = app.get(&format!("/api/v1/users/{id}"), Some(&admin)).await;
    assert_eq!(still_there.status, StatusCode::OK);

    app.cleanup().await;
}

#[tokio::test]
async fn non_admin_cannot_self_promote_via_update() {
    // A ROLE_USER can't even reach the update handler to set their own role to ROLE_ADMIN —
    // require_admin rejects the request before the role field in the body is ever read.
    let app = TestApp::spawn().await;
    let admin = register_admin(&app, "users-rbac-selfpromote-admin").await;
    let user_token = register_user(&app, "users-rbac-selfpromote-user").await;
    let all_users = app.get("/api/v1/users", Some(&admin)).await;
    let users: Vec<UserContract> = all_users.body_as();
    let self_id = users
        .iter()
        .find(|u| u.username.starts_with("user_users-rbac-selfpromote-user"))
        .expect("the registered user should be in the admin's user list")
        .id;

    let response = app
        .put(
            &format!("/api/v1/users/{self_id}"),
            Some(&user_token),
            json!({
                "username": "user_users-rbac-selfpromote-user",
                "role": "ROLE_ADMIN",
                "email": "users-rbac-selfpromote-user@example.com",
                "password": "whatever-123",
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);

    app.cleanup().await;
}
