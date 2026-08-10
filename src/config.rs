use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_access_token_expiration_seconds: i64,
    pub jwt_refresh_token_expiration_seconds: i64,
    pub seed_dev_data: bool,
    /// All three (username/email/password) must be set together to bootstrap the first admin
    /// account on startup — see `bootstrap_admin.rs`. Unset by default: there's no predictable
    /// default admin/password shipped, so an operator has to deliberately opt in.
    pub bootstrap_admin_username: Option<String>,
    pub bootstrap_admin_email: Option<String>,
    pub bootstrap_admin_password: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8080),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            jwt_access_token_expiration_seconds: env::var("JWT_ACCESS_TOKEN_EXPIRATION_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(86400),
            jwt_refresh_token_expiration_seconds: env::var(
                "JWT_REFRESH_TOKEN_EXPIRATION_SECONDS",
            )
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(604800),
            seed_dev_data: env::var("SEED_DEV_DATA")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            bootstrap_admin_username: env::var("BOOTSTRAP_ADMIN_USERNAME").ok(),
            bootstrap_admin_email: env::var("BOOTSTRAP_ADMIN_EMAIL").ok(),
            bootstrap_admin_password: env::var("BOOTSTRAP_ADMIN_PASSWORD").ok(),
        }
    }
}
