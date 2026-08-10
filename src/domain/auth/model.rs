use serde::{Deserialize, Serialize};

use crate::domain::user::model::Role;

#[derive(Deserialize, Debug, Clone)]
pub struct RegisterRequest {
    pub username: String,
    pub role: Role,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AuthenticationRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct AuthenticationResponse {
    pub access: String,
    pub refresh: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    /// Random per-token identifier. `iat`/`exp` alone only have second resolution, so two
    /// tokens minted for the same user within the same second would otherwise be byte-identical
    /// — this keeps every issued token's string unique so the `token` table's revocation state
    /// per row stays unambiguous.
    pub jti: String,
}
