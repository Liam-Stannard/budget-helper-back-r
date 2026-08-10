use std::env;

use config::{Config as ConfigLoader, File, FileFormat};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_access_token_expiration_seconds: i64,
    pub jwt_refresh_token_expiration_seconds: i64,
    pub seed_dev_data: bool,
    /// Which profile this was loaded from (`dev`, `prod`, ...) — see `load`. Not read from
    /// YAML itself; filled in after deserialization from `APP_ENV`.
    #[serde(default)]
    pub profile: String,
    /// All three (username/email/password) must be set together to bootstrap the first admin
    /// account on startup — see `bootstrap_admin.rs`. Only ever honored in the `dev` profile
    /// (see `main.rs`); a production admin is inserted directly into the database instead.
    pub bootstrap_admin_username: Option<String>,
    pub bootstrap_admin_email: Option<String>,
    pub bootstrap_admin_password: Option<String>,
}

impl Config {
    /// Loads layered YAML config: `config/application.yaml` (base, always required) overlaid
    /// by `config/application-{APP_ENV}.yaml` (profile-specific, optional — later keys win),
    /// where `APP_ENV` defaults to `dev`. Production supplies its own `config/
    /// application-prod.yaml` (gitignored; see `application-prod.yaml.example`) rather than
    /// committing secrets.
    pub fn load() -> Self {
        let profile = env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());

        let raw = ConfigLoader::builder()
            .add_source(File::new("config/application.yaml", FileFormat::Yaml))
            .add_source(
                File::new(&format!("config/application-{profile}.yaml"), FileFormat::Yaml)
                    .required(false),
            )
            .build()
            .unwrap_or_else(|e| panic!("failed to load configuration for profile {profile:?}: {e}"));

        let mut config: Config = raw.try_deserialize().unwrap_or_else(|e| {
            panic!("configuration for profile {profile:?} does not match the expected shape: {e}")
        });
        config.profile = profile;
        config
    }
}
