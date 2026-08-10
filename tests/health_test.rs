mod common;

use axum::http::StatusCode;
use common::TestApp;

#[tokio::test]
async fn health_check_returns_ok() {
    let app = TestApp::spawn().await;

    let response = app.get("/health", None).await;
    assert_eq!(response.status, StatusCode::OK);

    app.cleanup().await;
}
