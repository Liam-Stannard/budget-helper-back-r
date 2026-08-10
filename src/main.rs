mod app;
mod config;
mod domain;
mod error;
mod middleware;
mod state;

use std::sync::Arc;

use sqlx::mysql::MySqlPoolOptions;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();

    let db = MySqlPoolOptions::new()
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let state = AppState {
        db,
        config: Arc::new(config.clone()),
    };

    let router = app::build_router(state);

    let listener = TcpListener::bind((config.host.as_str(), config.port)).await?;
    tracing::info!("listening on {}:{}", config.host, config.port);

    axum::serve(listener, router).await?;

    Ok(())
}
