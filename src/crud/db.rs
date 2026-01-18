use anyhow::Result;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use std::str::FromStr;

use crate::utils::get_data_dir;

#[derive(Clone)]
pub struct DB {
    pub(super) pool: SqlitePool,
}

impl DB {
    pub async fn new() -> Result<Self> {
        let data_dir = get_data_dir()?;
        let db_path = data_dir.join("cards.db");

        let options =
            SqliteConnectOptions::from_str(&db_path.to_string_lossy())?.create_if_missing(true);

        Self::connect(options).await
    }
    async fn connect(options: SqliteConnectOptions) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }
}

#[cfg(test)]
impl DB {
    pub async fn new_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        Self::connect(options).await
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;

    use super::*;

    #[tokio::test]
    async fn test_db_connection() {
        let db_path = temp_dir().join("cards.db");

        let options = SqliteConnectOptions::from_str(&db_path.to_string_lossy())
            .unwrap()
            .create_if_missing(true);

        DB::connect(options).await.unwrap();
    }
}
