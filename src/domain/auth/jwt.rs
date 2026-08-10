use argon2::password_hash::rand_core::{OsRng, RngCore};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};

use crate::{config::Config, error::AppError};

use super::model::Claims;

fn generate_jti() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn build_token(sub: &str, expiration_seconds: i64, secret: &str) -> Result<String, AppError> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: sub.to_string(),
        iat: now,
        exp: now + expiration_seconds as usize,
        jti: generate_jti(),
    };

    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

pub fn generate_access_token(username: &str, config: &Config) -> Result<String, AppError> {
    build_token(
        username,
        config.jwt_access_token_expiration_seconds,
        &config.jwt_secret,
    )
}

pub fn generate_refresh_token(username: &str, config: &Config) -> Result<String, AppError> {
    build_token(
        username,
        config.jwt_refresh_token_expiration_seconds,
        &config.jwt_secret,
    )
}

pub fn decode_claims(token: &str, secret: &str) -> Result<Claims, AppError> {
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AppError::Unauthorized)?
    .claims;

    Ok(claims)
}
