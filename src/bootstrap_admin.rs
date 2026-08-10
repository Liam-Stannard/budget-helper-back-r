//! Bootstraps the first `ROLE_ADMIN` account on startup.
//!
//! `/auth/register` can never create an admin (it hardcodes every self-registered account to
//! `ROLE_USER`), and `/api/v1/users` now requires an existing admin to call it — so without
//! this, a fresh deployment has no way to ever produce one. Runs on every startup but is a
//! no-op as soon as any admin exists, so `BOOTSTRAP_ADMIN_*` is safe to leave set permanently;
//! it only ever fires once, on the deployment's first run.

use sqlx::mysql::MySqlDatabaseError;
use sqlx::MySqlPool;

use crate::config::Config;
use crate::domain::user::{
    model::{Role, User},
    password, repository,
};
use crate::error::AppError;

pub async fn run(pool: &MySqlPool, config: &Config) -> Result<(), AppError> {
    let existing_admin: Option<i64> = sqlx::query_scalar("SELECT id FROM user WHERE role = ? LIMIT 1")
        .bind(Role::RoleAdmin)
        .fetch_optional(pool)
        .await?;

    if existing_admin.is_some() {
        return Ok(());
    }

    let (username, email, plaintext_password) = match (
        &config.bootstrap_admin_username,
        &config.bootstrap_admin_email,
        &config.bootstrap_admin_password,
    ) {
        (Some(username), Some(email), Some(password)) => (username, email, password),
        (None, None, None) => {
            tracing::warn!(
                "no ROLE_ADMIN account exists and BOOTSTRAP_ADMIN_USERNAME / \
                 BOOTSTRAP_ADMIN_EMAIL / BOOTSTRAP_ADMIN_PASSWORD are not set — /api/v1/users \
                 will be unreachable until an admin is created some other way (e.g. promoting \
                 an existing row directly in the database) or those env vars are set and the \
                 app restarted"
            );
            return Ok(());
        }
        _ => {
            tracing::warn!(
                "BOOTSTRAP_ADMIN_USERNAME, BOOTSTRAP_ADMIN_EMAIL and BOOTSTRAP_ADMIN_PASSWORD \
                 must all be set together to bootstrap an admin account — one or two are set \
                 but not all three, so skipping"
            );
            return Ok(());
        }
    };

    let new_admin = User {
        id: 0,
        username: username.clone(),
        role: Role::RoleAdmin,
        email: email.clone(),
        password: password::hash_password(plaintext_password)?,
    };

    match repository::create(pool, &new_admin).await {
        Ok(created) => {
            tracing::info!(username = %created.username, "bootstrapped first admin account");
            Ok(())
        }
        // A duplicate username/email most likely means BOOTSTRAP_ADMIN_* collides with an
        // already-registered (non-admin) account. Don't fail startup over it — just leave the
        // system without an admin, same as if the env vars had been unset.
        Err(AppError::Database(e))
            if e.as_database_error()
                .and_then(|db_err| db_err.try_downcast_ref::<MySqlDatabaseError>())
                .is_some_and(|db_err| db_err.number() == 1062) =>
        {
            tracing::warn!(
                "BOOTSTRAP_ADMIN_USERNAME/EMAIL match an existing account that isn't an admin \
                 — skipping bootstrap rather than failing startup; promote that account \
                 manually or change the bootstrap credentials"
            );
            Ok(())
        }
        Err(e) => Err(e),
    }
}
