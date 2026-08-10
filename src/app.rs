use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::trace::TraceLayer;

use crate::{
    domain::{account, auth, transaction, user},
    middleware::auth::{require_admin, require_auth},
    state::AppState,
};

async fn health() -> &'static str {
    "ok"
}

pub fn build_router(state: AppState) -> Router {
    let public = Router::new()
        .route("/health", get(health))
        .route("/api/v1/auth/register", post(auth::handler::register))
        .route(
            "/api/v1/auth/authenticate",
            post(auth::handler::authenticate),
        )
        .route("/api/v1/auth/refresh", post(auth::handler::refresh));

    let protected = Router::new()
        .route(
            "/api/v1/account",
            get(account::handler::list).post(account::handler::create),
        )
        .route(
            "/api/v1/account/id/{id}",
            get(account::handler::get_by_id)
                .put(account::handler::update)
                .delete(account::handler::delete),
        )
        .route(
            "/api/v1/transaction",
            get(transaction::handler::list).post(transaction::handler::create),
        )
        .route(
            "/api/v1/transaction/id/{id}",
            get(transaction::handler::get_by_id)
                .put(transaction::handler::update)
                .delete(transaction::handler::delete),
        )
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    // `require_admin` reads the `Claims` request extension that `require_auth` inserts, so it
    // must run *after* require_auth on the way in. `Router::layer` wraps outside whatever was
    // there before, i.e. the layer added *last* runs *first* — so require_admin is added
    // first (innermost, closest to the handler) and require_auth last (outermost).
    let admin_only = Router::new()
        .route(
            "/api/v1/users",
            get(user::handler::list).post(user::handler::create),
        )
        .route(
            "/api/v1/users/{id}",
            get(user::handler::get_by_id)
                .put(user::handler::update)
                .delete(user::handler::delete),
        )
        .layer(middleware::from_fn_with_state(state.clone(), require_admin))
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    public
        .merge(protected)
        .merge(admin_only)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
