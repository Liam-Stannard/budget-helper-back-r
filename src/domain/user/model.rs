use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sqlx::{
    encode::IsNull,
    error::BoxDynError,
    mysql::{MySql, MySqlTypeInfo, MySqlValueRef},
    Decode, Encode, Type,
};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Role {
    RoleUser,
    RoleAdmin,
}

impl Role {
    fn as_str(&self) -> &'static str {
        match self {
            Role::RoleUser => "ROLE_USER",
            Role::RoleAdmin => "ROLE_ADMIN",
        }
    }
}

impl FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ROLE_USER" => Ok(Role::RoleUser),
            "ROLE_ADMIN" => Ok(Role::RoleAdmin),
            other => Err(format!("unknown role: {other}")),
        }
    }
}

// MySQL's ENUM column type carries per-column member metadata that the sqlx derive macro's
// generic `MySqlTypeInfo::__enum()` can't match, so `role` is stored/compared as a plain
// VARCHAR and mapped to/from `Role` manually here instead of relying on `#[derive(sqlx::Type)]`.
impl Type<MySql> for Role {
    fn type_info() -> MySqlTypeInfo {
        <String as Type<MySql>>::type_info()
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        <String as Type<MySql>>::compatible(ty)
    }
}

impl Encode<'_, MySql> for Role {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        <&str as Encode<MySql>>::encode_by_ref(&self.as_str(), buf)
    }
}

impl<'r> Decode<'r, MySql> for Role {
    fn decode(value: MySqlValueRef<'r>) -> Result<Self, BoxDynError> {
        let s = <&str as Decode<MySql>>::decode(value)?;
        s.parse::<Role>().map_err(Into::into)
    }
}

#[derive(sqlx::FromRow, Serialize, Debug, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub role: Role,
    pub email: String,
    #[serde(skip_serializing)]
    pub password: String,
}

// `id` is server-assigned (create) or taken from the path (update), so create/update take this
// instead of `User` directly — same reasoning as `account::model::AccountRequest`.
#[derive(Deserialize, Debug, Clone)]
pub struct UserRequest {
    pub username: String,
    pub role: Role,
    pub email: String,
    pub password: String,
}
