use once_cell::sync::OnceCell;
use sqlx::{migrate::MigrateDatabase, Pool, Sqlite, SqlitePool};

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct User {
    id: i64,
    name: String,
    pub osu_id: i64,
}

#[derive(Debug)]
pub enum UserError {
    DatabaseError(String),
    UserNotFound,
}

static DB_POOL: OnceCell<Pool<Sqlite>> = OnceCell::new();

pub async fn initialize_db() -> Result<(), sqlx::Error> {
    const DB_URL: &str = "sqlite:database.db";

    if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
        Sqlite::create_database(DB_URL).await?;
        println!("Database created");
    }

    let pool = SqlitePool::connect(DB_URL).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    DB_POOL.set(pool).expect("Failed to set DB pool");
    Ok(())
}

pub async fn insert_user(dc_id: i64, name: &str, osu_id: i64) -> Result<(), sqlx::Error> {
    let pool = DB_POOL.get().ok_or_else(|| sqlx::Error::PoolClosed)?;

    sqlx::query!(
        "INSERT INTO users (id, name, osu_id) VALUES (?, ?, ?)",
        dc_id,
        name,
        osu_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_user_by_id(id: i64) -> Result<User, UserError> {
    let pool = DB_POOL
        .get()
        .ok_or_else(|| UserError::DatabaseError("Database pool not initialized".to_string()))?;

    sqlx::query_as!(User, "SELECT * FROM users WHERE id = ?", id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            println!("Database error: {:?}", e);
            UserError::DatabaseError(e.to_string())
        })?
        .ok_or(UserError::UserNotFound)
}
