//! Contract tests for `/api/v1/auth/*`. See docs/api-spec.md.

mod common;

use axum::http::{Method, StatusCode};
use common::{register_user, TestApp};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthResponseContract {
    access: String,
    refresh: String,
}

fn looks_like_jwt(token: &str) -> bool {
    token.split('.').count() == 3
}

#[tokio::test]
async fn register_returns_access_and_refresh_tokens() {
    let app = TestApp::spawn().await;

    let response = app
        .post(
            "/api/v1/auth/register",
            None,
            json!({
                "username": "alice",
                "role": "ROLE_USER",
                "email": "alice@example.com",
                "password": "hunter2-hunter2",
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: AuthResponseContract = response.body_as();
    assert!(looks_like_jwt(&body.access));
    assert!(looks_like_jwt(&body.refresh));
    assert_ne!(body.access, body.refresh);

    app.cleanup().await;
}

#[tokio::test]
async fn register_duplicate_username_returns_409() {
    let app = TestApp::spawn().await;

    let payload = json!({
        "username": "dupe",
        "role": "ROLE_USER",
        "email": "dupe1@example.com",
        "password": "hunter2-hunter2",
    });
    let first = app.post("/api/v1/auth/register", None, payload.clone()).await;
    assert_eq!(first.status, StatusCode::OK);

    // Same username, different email — should still collide on the username unique constraint.
    let mut second_payload = payload;
    second_payload["email"] = json!("dupe2@example.com");
    let second = app.post("/api/v1/auth/register", None, second_payload).await;
    assert_eq!(second.status, StatusCode::CONFLICT);

    app.cleanup().await;
}

#[tokio::test]
async fn authenticate_with_correct_credentials_returns_tokens() {
    let app = TestApp::spawn().await;
    app.post(
        "/api/v1/auth/register",
        None,
        json!({
            "username": "bob",
            "role": "ROLE_USER",
            "email": "bob@example.com",
            "password": "correct-horse-battery",
        }),
    )
    .await;

    let response = app
        .post(
            "/api/v1/auth/authenticate",
            None,
            json!({ "username": "bob", "password": "correct-horse-battery" }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: AuthResponseContract = response.body_as();
    assert!(looks_like_jwt(&body.access));

    app.cleanup().await;
}

#[tokio::test]
async fn authenticate_with_wrong_password_returns_401() {
    let app = TestApp::spawn().await;
    app.post(
        "/api/v1/auth/register",
        None,
        json!({
            "username": "carol",
            "role": "ROLE_USER",
            "email": "carol@example.com",
            "password": "the-real-password",
        }),
    )
    .await;

    let response = app
        .post(
            "/api/v1/auth/authenticate",
            None,
            json!({ "username": "carol", "password": "wrong-password" }),
        )
        .await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn authenticate_with_unknown_username_returns_401() {
    let app = TestApp::spawn().await;

    let response = app
        .post(
            "/api/v1/auth/authenticate",
            None,
            json!({ "username": "nobody-here", "password": "whatever" }),
        )
        .await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn logging_in_again_revokes_the_previous_access_token() {
    let app = TestApp::spawn().await;
    let register = app
        .post(
            "/api/v1/auth/register",
            None,
            json!({
                "username": "dave",
                "role": "ROLE_USER",
                "email": "dave@example.com",
                "password": "first-session-pw",
            }),
        )
        .await;
    let first_token = register.json["access"].as_str().unwrap().to_string();

    // First token works on a protected route right after registering.
    let ok = app.get("/api/v1/account", Some(&first_token)).await;
    assert_eq!(ok.status, StatusCode::OK);

    // Logging in again mints a second token and, per spec, revokes the first.
    let login = app
        .post(
            "/api/v1/auth/authenticate",
            None,
            json!({ "username": "dave", "password": "first-session-pw" }),
        )
        .await;
    let second_token = login.json["access"].as_str().unwrap().to_string();
    assert_ne!(first_token, second_token);

    let stale = app.get("/api/v1/account", Some(&first_token)).await;
    assert_eq!(stale.status, StatusCode::UNAUTHORIZED);

    let fresh = app.get("/api/v1/account", Some(&second_token)).await;
    assert_eq!(fresh.status, StatusCode::OK);

    app.cleanup().await;
}

#[tokio::test]
async fn refresh_returns_a_new_access_token_and_echoes_the_refresh_token() {
    let app = TestApp::spawn().await;
    let register = app
        .post(
            "/api/v1/auth/register",
            None,
            json!({
                "username": "erin",
                "role": "ROLE_USER",
                "email": "erin@example.com",
                "password": "refresh-me-please",
            }),
        )
        .await;
    let original_access = register.json["access"].as_str().unwrap().to_string();
    let refresh_token = register.json["refresh"].as_str().unwrap().to_string();

    let response = app
        .request(Method::POST, "/api/v1/auth/refresh", Some(&refresh_token), None)
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body: AuthResponseContract = response.body_as();
    assert_eq!(body.refresh, refresh_token, "refresh token should be echoed back unchanged");
    assert_ne!(body.access, original_access, "a fresh access token should be minted");

    // The new access token should work; the pre-refresh one should have been revoked.
    let fresh = app.get("/api/v1/account", Some(&body.access)).await;
    assert_eq!(fresh.status, StatusCode::OK);
    let stale = app.get("/api/v1/account", Some(&original_access)).await;
    assert_eq!(stale.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn refresh_without_a_bearer_header_returns_401() {
    let app = TestApp::spawn().await;

    let response = app.request(Method::POST, "/api/v1/auth/refresh", None, None).await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn refresh_with_a_malformed_token_returns_401() {
    let app = TestApp::spawn().await;

    let response = app
        .request(
            Method::POST,
            "/api/v1/auth/refresh",
            Some("not-a-real-jwt"),
            None,
        )
        .await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn protected_route_without_a_token_returns_401() {
    let app = TestApp::spawn().await;

    let response = app.get("/api/v1/account", None).await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn protected_route_with_a_garbage_token_returns_401() {
    let app = TestApp::spawn().await;

    let response = app.get("/api/v1/account", Some("garbage.token.value")).await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn error_body_matches_the_shared_envelope() {
    let app = TestApp::spawn().await;

    let response = app.get("/api/v1/account", None).await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    assert!(response.json["message"].is_string());
    assert_eq!(response.json["status"].as_u64(), Some(401));
    assert!(response.json["timestamp"].is_string());

    app.cleanup().await;
}

#[tokio::test]
async fn register_helper_produces_a_usable_token() {
    // Sanity-checks the shared test helper other domain test files rely on.
    let app = TestApp::spawn().await;
    let token = register_user(&app, "helper-check").await;

    let response = app
        .request(
            Method::GET,
            "/api/v1/account",
            Some(&token),
            None,
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json["accountList"].as_array().unwrap().len(), 0);

    app.cleanup().await;
}
