use anyhow::Result;
use directories::ProjectDirs;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use std::str::FromStr;

use anyhow::anyhow;

#[derive(Clone)]
pub struct DB {
    pub(super) pool: SqlitePool,
}

impl DB {
    pub async fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("", "", "repeater")
            .ok_or_else(|| anyhow!("Could not determine project directory"))?;

        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;

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
