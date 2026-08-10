// Each `tests/*.rs` file is compiled as its own binary and only uses a subset of this shared
// module's surface, so the compiler flags the rest as dead code per-binary even though every
// item here is used by at least one test file.
#![allow(dead_code)]

//! Shared integration-test harness.
//!
//! Each test spins up a throwaway MySQL/MariaDB database on the same local server used for dev
//! (`start.sh`), migrates it, and serves the real `axum::Router` in-process via
//! `tower::ServiceExt::oneshot` — no TCP socket, no mocking. This means tests exercise the
//! actual routing, extractors, auth middleware, and SQL, which is what let the transaction/
//! account `account_id` vs nested `account` contract bugs slip through originally: nothing
//! ever asserted on the JSON shape actually sent over the wire.
//!
//! Requires a reachable MariaDB/MySQL server (see `start.sh`); reads connection details from
//! the same layered YAML config the app itself loads (`config/application.yaml` overlaid by
//! `config/application-${APP_ENV}.yaml`, defaulting to the `dev` profile — see
//! `src/config.rs`).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    Router,
};
use budget_helper_back_r::{app, config::Config, state::AppState};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sqlx::{mysql::MySqlPoolOptions, Executor};
use tower::ServiceExt;

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TestApp {
    router: Router,
    /// Kept for fixture setup that has no route to reach through HTTP — e.g. seeding an admin
    /// user, since `/auth/register` hardcodes every new account to `ROLE_USER` (see
    /// `seed_admin` below) and `/api/v1/users` now requires an admin to call it in the first
    /// place.
    state: AppState,
    admin_url: String,
    db_name: String,
}

pub struct TestResponse {
    pub status: StatusCode,
    pub json: Value,
}

impl TestResponse {
    /// Deserializes the body into `T`. Test-local contract types should derive
    /// `#[serde(deny_unknown_fields)]` so this fails on extra fields, not just missing ones —
    /// that's what makes it a check of the *exact* wire shape rather than a subset match.
    pub fn body_as<T: DeserializeOwned>(&self) -> T {
        serde_json::from_value(self.json.clone()).unwrap_or_else(|e| {
            panic!(
                "response body did not match the expected contract: {e}\nbody: {}",
                self.json
            )
        })
    }
}

impl TestApp {
    pub async fn spawn() -> Self {
        let base_url = Config::load().database_url;
        let admin_url = base_url
            .rsplit_once('/')
            .map(|(prefix, _)| prefix.to_string())
            .unwrap_or_else(|| base_url.clone());

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let db_name = format!("budget_helper_test_{}_{nanos}_{n}", std::process::id());

        let admin_pool = MySqlPoolOptions::new()
            .connect(&admin_url)
            .await
            .expect(
                "could not connect to local MySQL/MariaDB — is it running? see start.sh, \
                 and check database_url in config/application-dev.yaml",
            );

        admin_pool
            .execute(format!("CREATE DATABASE `{db_name}`").as_str())
            .await
            .expect("create ephemeral test database");

        let db_url = format!("{admin_url}/{db_name}");
        let db = MySqlPoolOptions::new()
            .connect(&db_url)
            .await
            .expect("connect to freshly created test database");

        sqlx::migrate!("./migrations")
            .run(&db)
            .await
            .expect("run migrations against test database");

        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 0,
            database_url: db_url,
            jwt_secret: "test-only-secret-do-not-use-outside-tests-0123456789".to_string(),
            jwt_access_token_expiration_seconds: 3600,
            jwt_refresh_token_expiration_seconds: 3600,
            seed_dev_data: false,
            profile: "test".to_string(),
            bootstrap_admin_username: None,
            bootstrap_admin_email: None,
            bootstrap_admin_password: None,
        };

        let state = AppState {
            db,
            config: Arc::new(config),
        };
        let router = app::build_router(state.clone());

        TestApp {
            router,
            state,
            admin_url,
            db_name,
        }
    }

    /// Exposes the test database pool for fixtures/checks that need to bypass HTTP — e.g.
    /// calling `bootstrap_admin::run` directly, or asserting on rows the API doesn't expose.
    pub fn db(&self) -> &sqlx::MySqlPool {
        &self.state.db
    }

    /// Exposes the app's `Config` (as built by `spawn`) so tests can clone and tweak it — e.g.
    /// setting `bootstrap_admin_*` fields, which `spawn` otherwise leaves unset.
    pub fn config(&self) -> &Config {
        &self.state.config
    }

    /// Drops the ephemeral database. Call at the end of every test; a test that panics before
    /// reaching this just leaves an orphaned `budget_helper_test_*` database behind, which is
    /// harmless (uniquely named, never reused) but worth clearing out periodically by hand.
    pub async fn cleanup(self) {
        if let Ok(admin_pool) = MySqlPoolOptions::new().connect(&self.admin_url).await {
            let _ = admin_pool
                .execute(format!("DROP DATABASE IF EXISTS `{}`", self.db_name).as_str())
                .await;
        }
    }

    pub async fn request(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> TestResponse {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }

        let body = match body {
            Some(value) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                Body::from(serde_json::to_vec(&value).unwrap())
            }
            None => Body::empty(),
        };

        let request = builder.body(body).unwrap();

        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("router call should not fail at the transport level");

        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("collect response body")
            .to_bytes();

        // Not every response is JSON (e.g. `/health` is plain text) — fall back to treating the
        // raw text as a JSON string rather than failing, so callers can still assert on it.
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
        };

        TestResponse { status, json }
    }

    pub async fn get(&self, path: &str, token: Option<&str>) -> TestResponse {
        self.request(Method::GET, path, token, None).await
    }

    pub async fn post(&self, path: &str, token: Option<&str>, body: Value) -> TestResponse {
        self.request(Method::POST, path, token, Some(body)).await
    }

    pub async fn put(&self, path: &str, token: Option<&str>, body: Value) -> TestResponse {
        self.request(Method::PUT, path, token, Some(body)).await
    }

    pub async fn delete(&self, path: &str, token: Option<&str>) -> TestResponse {
        self.request(Method::DELETE, path, token, None).await
    }
}

/// Registers a fresh `ROLE_USER` through the real `/auth/register` endpoint and returns their
/// access token. `unique` must be unique per call within a test (username/email are
/// unique-constrained).
pub async fn register_user(app: &TestApp, unique: &str) -> String {
    register_with_role(app, unique, "ROLE_USER").await
}

/// Seeds a `ROLE_ADMIN` user directly through the repository layer and returns their access
/// token, bypassing HTTP entirely.
///
/// `/auth/register` hardcodes every self-registered account to `ROLE_USER` (the `role` field on
/// `RegisterRequest` is accepted but never read — see docs/api-spec.md's "Known gaps") and
/// `/api/v1/users` now requires an admin to call it, so there is currently no HTTP path that
/// produces the first admin account. This is a real gap in the product, not just a test
/// inconvenience: as it stands, nothing can ever populate an admin through the running API.
pub async fn register_admin(app: &TestApp, unique: &str) -> String {
    use budget_helper_back_r::domain::{
        auth::jwt,
        token,
        user::{
            model::{Role, User},
            password, repository as user_repository,
        },
    };

    let user = User {
        id: 0,
        username: format!("admin_{unique}"),
        role: Role::RoleAdmin,
        email: format!("{unique}-admin@example.com"),
        password: password::hash_password("correct-horse-battery-staple").unwrap(),
    };
    let created = user_repository::create(&app.state.db, &user)
        .await
        .expect("seed admin user");

    let access = jwt::generate_access_token(&created.username, &app.state.config)
        .expect("mint access token for seeded admin");
    token::repository::create(&app.state.db, &access, created.id)
        .await
        .expect("store seeded admin's access token");

    access
}

async fn register_with_role(app: &TestApp, unique: &str, role: &str) -> String {
    let response = app
        .post(
            "/api/v1/auth/register",
            None,
            serde_json::json!({
                "username": format!("user_{unique}"),
                "role": role,
                "email": format!("{unique}@example.com"),
                "password": "correct-horse-battery-staple",
            }),
        )
        .await;

    assert_eq!(
        response.status,
        StatusCode::OK,
        "register setup failed: {:?}",
        response.json
    );

    response.json["access"]
        .as_str()
        .expect("register response should contain an access token")
        .to_string()
}
