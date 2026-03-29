use anyhow::Result;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::sync::types::{SyncCard, remote_wins};

const SERVER_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS sessions (
    token TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS cards (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    card_hash TEXT NOT NULL,
    last_reviewed_at TEXT,
    stability REAL,
    difficulty REAL,
    interval_raw REAL,
    interval_days INTEGER,
    due_date TEXT,
    review_count INTEGER NOT NULL,
    added_at TEXT NOT NULL,
    version BIGINT NOT NULL DEFAULT 1,
    PRIMARY KEY (user_id, card_hash)
);

CREATE INDEX IF NOT EXISTS idx_server_cards_version ON cards(user_id, version);
"#;

#[derive(Clone)]
pub struct ServerDB {
    pub pool: PgPool,
}

impl ServerDB {
    pub async fn new(db_uri: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(db_uri)
            .await?;

        // Run schema creation
        sqlx::raw_sql(SERVER_SCHEMA).execute(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn create_user(&self, username: &str, password_hash: &str) -> Result<Uuid> {
        let row = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO users (username, password_hash) VALUES ($1, $2) RETURNING id",
        )
        .bind(username)
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<(Uuid, String)>> {
        let row = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, password_hash FROM users WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn create_session(&self, user_id: Uuid, token: &str) -> Result<()> {
        sqlx::query("INSERT INTO sessions (token, user_id) VALUES ($1, $2)")
            .bind(token)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_user_id_by_token(&self, token: &str) -> Result<Option<Uuid>> {
        let row = sqlx::query_scalar::<_, Uuid>(
            "SELECT user_id FROM sessions WHERE token = $1 \
             AND created_at > now() - interval '30 days'",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Push cards with last-write-wins. Returns number of cards actually updated.
    pub async fn push_cards(&self, user_id: Uuid, cards: &[SyncCard]) -> Result<i64> {
        let mut tx = self.pool.begin().await?;
        let mut updated = 0i64;

        // Get current max version for this user
        let max_version: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM cards WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&mut *tx)
                .await?;

        let mut next_version = max_version + 1;

        for card in cards {
            // Check existing card
            let existing = sqlx::query_as::<_, (Option<String>,)>(
                "SELECT last_reviewed_at FROM cards WHERE user_id = $1 AND card_hash = $2",
            )
            .bind(user_id)
            .bind(&card.card_hash)
            .fetch_optional(&mut *tx)
            .await?;

            match existing {
                Some((existing_reviewed_at,)) => {
                    if remote_wins(&card.last_reviewed_at, &existing_reviewed_at) {
                        sqlx::query(
                            "UPDATE cards SET last_reviewed_at = $1, stability = $2, difficulty = $3, \
                             interval_raw = $4, interval_days = $5, due_date = $6, review_count = $7, \
                             version = $8 WHERE user_id = $9 AND card_hash = $10",
                        )
                        .bind(&card.last_reviewed_at)
                        .bind(card.stability)
                        .bind(card.difficulty)
                        .bind(card.interval_raw)
                        .bind(card.interval_days.map(|v| v as i32))
                        .bind(&card.due_date)
                        .bind(card.review_count as i32)
                        .bind(next_version)
                        .bind(user_id)
                        .bind(&card.card_hash)
                        .execute(&mut *tx)
                        .await?;

                        next_version += 1;
                        updated += 1;
                    }
                }
                None => {
                    // Insert new card
                    sqlx::query(
                        "INSERT INTO cards (user_id, card_hash, last_reviewed_at, stability, difficulty, \
                         interval_raw, interval_days, due_date, review_count, added_at, version) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                    )
                    .bind(user_id)
                    .bind(&card.card_hash)
                    .bind(&card.last_reviewed_at)
                    .bind(card.stability)
                    .bind(card.difficulty)
                    .bind(card.interval_raw)
                    .bind(card.interval_days.map(|v| v as i32))
                    .bind(&card.due_date)
                    .bind(card.review_count as i32)
                    .bind(&card.added_at)
                    .bind(next_version)
                    .execute(&mut *tx)
                    .await?;

                    next_version += 1;
                    updated += 1;
                }
            }
        }

        tx.commit().await?;
        Ok(updated)
    }

    /// Pull cards changed since a given version.
    pub async fn pull_cards(
        &self,
        user_id: Uuid,
        since_version: i64,
    ) -> Result<(Vec<SyncCard>, i64)> {
        let rows = sqlx::query_as::<_, (String, Option<String>, Option<f32>, Option<f32>, Option<f32>, Option<i32>, Option<String>, i32, String, i64)>(
            "SELECT card_hash, last_reviewed_at, stability, difficulty, interval_raw, interval_days, \
             due_date, review_count, added_at, version \
             FROM cards WHERE user_id = $1 AND version > $2 ORDER BY version ASC",
        )
        .bind(user_id)
        .bind(since_version)
        .fetch_all(&self.pool)
        .await?;

        let mut latest_version = since_version;
        let cards: Vec<SyncCard> = rows
            .into_iter()
            .map(
                |(
                    card_hash,
                    last_reviewed_at,
                    stability,
                    difficulty,
                    interval_raw,
                    interval_days,
                    due_date,
                    review_count,
                    added_at,
                    version,
                )| {
                    if version > latest_version {
                        latest_version = version;
                    }
                    SyncCard {
                        card_hash,
                        last_reviewed_at,
                        stability: stability.map(|v| v as f64),
                        difficulty: difficulty.map(|v| v as f64),
                        interval_raw: interval_raw.map(|v| v as f64),
                        interval_days: interval_days.map(|v| v as i64),
                        due_date,
                        review_count: review_count as i64,
                        added_at,
                    }
                },
            )
            .collect();

        Ok((cards, latest_version))
    }

    pub async fn get_status(&self, user_id: Uuid) -> Result<(i64, i64)> {
        let card_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cards WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        let latest_version: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM cards WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;

        Ok((card_count, latest_version))
    }
}
