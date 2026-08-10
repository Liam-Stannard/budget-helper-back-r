//! Tests for `bootstrap_admin::run` — the startup step that creates the first `ROLE_ADMIN`
//! account from `BOOTSTRAP_ADMIN_*` env vars, since nothing reachable over HTTP can ever
//! produce one otherwise (`/auth/register` hardcodes `ROLE_USER`, and `/api/v1/users` is
//! itself admin-gated). See docs/api-spec.md's "Known gaps".

mod common;

use axum::http::StatusCode;
use budget_helper_back_r::{bootstrap_admin, config::Config};
use common::{register_admin, TestApp};
use serde_json::json;

fn config_with_bootstrap(app: &TestApp, username: &str, email: &str, password: &str) -> Config {
    let mut config = app.config().clone();
    config.bootstrap_admin_username = Some(username.to_string());
    config.bootstrap_admin_email = Some(email.to_string());
    config.bootstrap_admin_password = Some(password.to_string());
    config
}

async fn count_admins(app: &TestApp) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM user WHERE role = 'ROLE_ADMIN'")
        .fetch_one(app.db())
        .await
        .unwrap()
}

#[tokio::test]
async fn creates_an_admin_when_none_exists_and_env_vars_are_set() {
    let app = TestApp::spawn().await;
    let config = config_with_bootstrap(&app, "bootstrap-admin", "bootstrap-admin@example.com", "a-strong-password");

    bootstrap_admin::run(app.db(), &config).await.expect("bootstrap should succeed");

    assert_eq!(count_admins(&app).await, 1);

    // The bootstrapped account should actually work end-to-end: log in, then hit an
    // admin-only route.
    let login = app
        .post(
            "/api/v1/auth/authenticate",
            None,
            json!({ "username": "bootstrap-admin", "password": "a-strong-password" }),
        )
        .await;
    assert_eq!(login.status, StatusCode::OK, "{:?}", login.json);
    let token = login.json["access"].as_str().unwrap();

    let users_response = app.get("/api/v1/users", Some(token)).await;
    assert_eq!(users_response.status, StatusCode::OK);

    app.cleanup().await;
}

#[tokio::test]
async fn is_a_noop_when_an_admin_already_exists() {
    let app = TestApp::spawn().await;
    // `register_admin` seeds an admin directly (the only way to get one today, per the doc
    // comment on it) — bootstrap should see that and back off, even with different env vars.
    register_admin(&app, "bootstrap-existing").await;
    assert_eq!(count_admins(&app).await, 1);

    let config = config_with_bootstrap(&app, "should-not-appear", "should-not-appear@example.com", "whatever-123");
    bootstrap_admin::run(app.db(), &config).await.expect("bootstrap should succeed (as a no-op)");

    assert_eq!(count_admins(&app).await, 1, "bootstrap should not create a second admin");

    let login = app
        .post(
            "/api/v1/auth/authenticate",
            None,
            json!({ "username": "should-not-appear", "password": "whatever-123" }),
        )
        .await;
    assert_eq!(
        login.status,
        StatusCode::UNAUTHORIZED,
        "the bootstrap credentials should never have been used to create an account"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn skips_when_no_bootstrap_env_vars_are_set() {
    let app = TestApp::spawn().await;
    let config = app.config().clone(); // bootstrap_admin_* are all None by default

    bootstrap_admin::run(app.db(), &config).await.expect("bootstrap should succeed (as a no-op)");

    assert_eq!(count_admins(&app).await, 0);

    app.cleanup().await;
}

#[tokio::test]
async fn skips_when_only_some_bootstrap_env_vars_are_set() {
    let app = TestApp::spawn().await;
    let mut config = app.config().clone();
    config.bootstrap_admin_username = Some("partial".to_string());
    config.bootstrap_admin_email = Some("partial@example.com".to_string());
    // password left unset

    bootstrap_admin::run(app.db(), &config).await.expect("bootstrap should succeed (as a no-op)");

    assert_eq!(count_admins(&app).await, 0);

    app.cleanup().await;
}

#[tokio::test]
async fn skips_gracefully_instead_of_erroring_on_a_username_collision() {
    let app = TestApp::spawn().await;

    // A non-admin already occupies the username/email the bootstrap config wants to use.
    let register = app
        .post(
            "/api/v1/auth/register",
            None,
            json!({
                "username": "taken",
                "role": "ROLE_USER",
                "email": "taken@example.com",
                "password": "whatever-123",
            }),
        )
        .await;
    assert_eq!(register.status, StatusCode::OK);

    let config = config_with_bootstrap(&app, "taken", "taken@example.com", "a-different-password");
    let result = bootstrap_admin::run(app.db(), &config).await;

    assert!(result.is_ok(), "a collision should be logged and skipped, not fail startup");
    assert_eq!(count_admins(&app).await, 0, "the existing account should not have been promoted");

    app.cleanup().await;
}
