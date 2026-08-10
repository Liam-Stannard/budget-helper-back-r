use serde::{Deserialize, Serialize};

#[derive(sqlx::Type, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[sqlx(rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Role {
    RoleUser,
    RoleAdmin,
}

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub role: Role,
    pub email: String,
    #[serde(skip_serializing)]
    pub password: String,
}
