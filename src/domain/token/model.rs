use serde::{Deserialize, Serialize};

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug, Clone)]
pub struct Token {
    pub id: i32,
    pub token: String,
    pub expired: bool,
    pub revoked: bool,
    pub user_id: i64,
}
