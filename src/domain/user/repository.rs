use sqlx::MySqlPool;

use crate::error::AppError;

use super::model::User;

const SELECT_FIELDS: &str = "id, username, role, email, password FROM user";

pub async fn find_all(pool: &MySqlPool) -> Result<Vec<User>, AppError> {
    let users = sqlx::query_as::<_, User>(&format!("SELECT {SELECT_FIELDS}"))
        .fetch_all(pool)
        .await?;
    Ok(users)
}

pub async fn find_by_id(pool: &MySqlPool, id: i64) -> Result<User, AppError> {
    let user = sqlx::query_as::<_, User>(&format!("SELECT {SELECT_FIELDS} WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(user)
}

pub async fn find_by_username(pool: &MySqlPool, username: &str) -> Result<User, AppError> {
    let user = sqlx::query_as::<_, User>(&format!("SELECT {SELECT_FIELDS} WHERE username = ?"))
        .bind(username)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(user)
}

pub async fn create(pool: &MySqlPool, user: &User) -> Result<User, AppError> {
    let result = sqlx::query("INSERT INTO user (username, role, email, password) VALUES (?, ?, ?, ?)")
        .bind(&user.username)
        .bind(user.role)
        .bind(&user.email)
        .bind(&user.password)
        .execute(pool)
        .await?;

    find_by_id(pool, result.last_insert_id() as i64).await
}

pub async fn update(pool: &MySqlPool, id: i64, user: &User) -> Result<User, AppError> {
    let result = sqlx::query("UPDATE user SET username = ?, role = ?, email = ?, password = ? WHERE id = ?")
        .bind(&user.username)
        .bind(user.role)
        .bind(&user.email)
        .bind(&user.password)
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    find_by_id(pool, id).await
}

pub async fn delete(pool: &MySqlPool, id: i64) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM user WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(())
}
