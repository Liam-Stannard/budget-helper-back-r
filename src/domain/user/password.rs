use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use crate::error::AppError;

pub fn hash_password(plain: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| AppError::PasswordHash(e.to_string()))?;
    Ok(hash.to_string())
}

pub fn verify_password(plain: &str, hash: &str) -> Result<bool, AppError> {
    let parsed_hash =
        PasswordHash::new(hash).map_err(|e| AppError::PasswordHash(e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed_hash)
        .is_ok())
}
