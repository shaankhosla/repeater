use anyhow::Result;
use directories::ProjectDirs;
use futures::TryStreamExt;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::anyhow;

use crate::card::Card;
use crate::fsrs::Performance;
use crate::fsrs::ReviewStatus;
use crate::fsrs::ReviewedPerformance;
use crate::fsrs::update_performance;
use crate::stats::CardStats;

pub struct DB {
    pool: SqlitePool,
}

impl DB {
    pub async fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("", "", "repeat")
            .ok_or_else(|| anyhow!("Could not determine project directory"))?;
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir)
            .map_err(|e| anyhow!("Failed to create data directory: {}", e))?;

        let db_path: PathBuf = data_dir.join("cards.db");
        let options =
            SqliteConnectOptions::from_str(&db_path.to_string_lossy())?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;

        sqlx::query("SELECT 1").execute(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn add_card(&self, card: &Card) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query!(
            r#"
        INSERT or ignore INTO cards (
            card_hash,
            added_at,
            last_reviewed_at,
            stability,
            difficulty,
            interval_raw,
            interval_days,
            due_date,
            review_count
        )
        VALUES (?, ?, NULL, NULL, NULL, NULL, 0, NULL, 0)
        "#,
            card.card_hash,
            now
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn add_cards_batch(&self, cards: &[Card]) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        let now = chrono::Utc::now().to_rfc3339();

        for card in cards {
            let added_at = now.clone();
            sqlx::query!(
                r#"
            INSERT or ignore INTO cards (
                card_hash,
                added_at,
                last_reviewed_at,
                stability,
                difficulty,
                interval_raw,
                interval_days,
                due_date,
                review_count
            )
            VALUES (?, ?, NULL, NULL, NULL, NULL, 0, NULL, 0)
            "#,
                card.card_hash,
                added_at
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn card_exists(&self, card: &Card) -> Result<bool> {
        let count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(1) as "count!: i64" FROM cards WHERE card_hash = ?"#,
            card.card_hash
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    pub async fn update_card_performance(
        &self,
        card: &Card,
        review_status: ReviewStatus,
    ) -> Result<bool> {
        let current_performance = self.get_card_performance(card).await?;
        let now = chrono::Utc::now();
        let new_performance = update_performance(current_performance, review_status, now);

        let interval_days = new_performance.interval_days as i64;
        let review_count = new_performance.review_count as i64;

        let result = sqlx::query!(
            r#"
            UPDATE cards
            SET
                last_reviewed_at = ?,
                stability = ?,
                difficulty = ?,
                interval_raw = ?,
                interval_days = ?,
                due_date = ?,
                review_count = ?
            WHERE card_hash = ?
            "#,
            new_performance.last_reviewed_at,
            new_performance.stability,
            new_performance.difficulty,
            new_performance.interval_raw,
            interval_days,
            new_performance.due_date,
            review_count,
            card.card_hash,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_card_performance(&self, card: &Card) -> Result<Performance> {
        let row = sqlx::query!(
            r#"
            SELECT
                last_reviewed_at as "last_reviewed_at?: chrono::DateTime<chrono::Utc>",
                stability as "stability?: f64",
                difficulty as "difficulty?: f64",
                interval_raw as "interval_raw?: f64",
                interval_days as "interval_days?: i64",
                due_date as "due_date?: chrono::DateTime<chrono::Utc>",
                review_count as "review_count!: i64"
            FROM cards
            WHERE card_hash = ?
            "#,
            card.card_hash
        )
        .fetch_one(&self.pool)
        .await?;

        let review_count: i64 = row.review_count;
        if review_count == 0 {
            return Ok(Performance::default());
        }
        let reviewed = ReviewedPerformance {
            last_reviewed_at: row
                .last_reviewed_at
                .ok_or_else(|| anyhow!("missing last_reviewed_at for card {}", card.card_hash))?,
            stability: row
                .stability
                .ok_or_else(|| anyhow!("missing stability for card {}", card.card_hash))?,
            difficulty: row
                .difficulty
                .ok_or_else(|| anyhow!("missing difficulty for card {}", card.card_hash))?,
            interval_raw: row
                .interval_raw
                .ok_or_else(|| anyhow!("missing interval_raw for card {}", card.card_hash))?,
            interval_days: row
                .interval_days
                .ok_or_else(|| anyhow!("missing interval_days for card {}", card.card_hash))?
                as usize,
            due_date: row
                .due_date
                .ok_or_else(|| anyhow!("missing due_date for card {}", card.card_hash))?,
            review_count: review_count as usize,
        };

        Ok(Performance::Reviewed(reviewed))
    }

    pub async fn due_today(
        &self,
        card_hashes: HashMap<String, Card>,
        card_limit: Option<usize>,
        new_card_limit: Option<usize>,
    ) -> Result<Vec<Card>> {
        let now = chrono::Utc::now().to_rfc3339();

        let mut rows = sqlx::query!(
            r#"
            SELECT card_hash, review_count as "review_count!: i64"
            FROM cards
            WHERE due_date <= ? OR due_date IS NULL
            "#,
            now
        )
        .fetch(&self.pool);
        let mut cards = Vec::new();
        let mut num_new_cards = 0;
        while let Some(row) = rows.try_next().await? {
            let card_hash = row.card_hash;
            if !card_hashes.contains_key(&card_hash) {
                continue;
            }

            if let Some(card) = card_hashes.get(&card_hash) {
                cards.push(card.clone());
                if row.review_count == 0 {
                    num_new_cards += 1;
                }
            }

            if let Some(card_limit) = card_limit
                && cards.len() >= card_limit
            {
                break;
            }
            if let Some(new_card_limit) = new_card_limit
                && num_new_cards >= new_card_limit
            {
                break;
            }
        }

        Ok(cards)
    }

    pub async fn collection_stats(&self, card_hashes: &HashMap<String, Card>) -> Result<CardStats> {
        let mut stats = CardStats {
            num_cards: card_hashes.len() as i64,
            ..Default::default()
        };

        let mut rows = sqlx::query_as!(
            CardStatsRow,
            r#"
            SELECT
                card_hash,
                review_count as "review_count!: i64",
                due_date as "due_date?: chrono::DateTime<chrono::Utc>",
                interval_raw as "interval_raw?: f64",
                difficulty as "difficulty?: f64",
                stability as "stability?: f64",
                last_reviewed_at as "last_reviewed_at?: chrono::DateTime<chrono::Utc>"
            FROM cards
            "#,
        )
        .fetch(&self.pool);

        while let Some(row) = rows.try_next().await? {
            stats.total_cards_in_db += 1;
            let card = match card_hashes.get(&row.card_hash) {
                Some(card) => card,
                None => continue,
            };
            stats.update(card, &row);
        }

        Ok(stats)
    }
}
pub struct CardStatsRow {
    pub card_hash: String,
    pub review_count: i64,
    pub due_date: Option<chrono::DateTime<chrono::Utc>>,
    pub interval_raw: Option<f64>,
    pub difficulty: Option<f64>,
    pub stability: Option<f64>,
    pub last_reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
}
