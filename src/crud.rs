use anyhow::Result;
use chrono::Duration;
use directories::ProjectDirs;
use futures::TryStreamExt;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use std::collections::HashMap;
use std::str::FromStr;

use anyhow::anyhow;

use crate::card::Card;
use crate::check_version::VersionUpdateStats;
use crate::fsrs::update_performance;
use crate::fsrs::{Performance, ReviewStage, ReviewStatus, ReviewedPerformance};
use crate::stats::CardStats;

const LEARN_AHEAD_THRESHOLD_MINS: Duration = Duration::minutes(20);

#[derive(Clone)]
pub struct DB {
    pool: SqlitePool,
}

impl DB {
    pub async fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("", "", "repeat")
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
    pub async fn get_version_update_information(&self) -> Result<VersionUpdateStats> {
        let stats = sqlx::query_as!(
            VersionUpdateStats,
            r#"
        SELECT
            last_prompted_at        AS "last_prompted_at?: chrono::DateTime<chrono::Utc>",
            last_version_check_at   AS "last_version_check_at?: chrono::DateTime<chrono::Utc>"
        FROM version_update
        "#
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(stats.unwrap_or_default())
    }
    pub async fn update_last_prompted_at(&self) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query!(
            r#"
            INSERT INTO version_update (id, last_prompted_at)
            VALUES (1, $1)
            ON CONFLICT (id)
            DO UPDATE SET last_prompted_at = EXCLUDED.last_prompted_at
            "#,
            now
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_last_version_check_at(&self) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query!(
            r#"
            INSERT INTO version_update (id, last_version_check_at)
            VALUES (1, $1)
            ON CONFLICT (id)
            DO UPDATE SET last_version_check_at = EXCLUDED.last_version_check_at
            "#,
            now
        )
        .execute(&self.pool)
        .await?;

        Ok(())
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
    ) -> Result<f64> {
        let current_performance = self.get_card_performance(card).await?;
        let now = chrono::Utc::now();
        let new_performance = update_performance(current_performance, review_status, now);

        let reviewed = match &new_performance {
            Performance::LearningA(r) | Performance::LearningB(r) | Performance::Review(r) => r,
            Performance::New => {
                return Err(anyhow!(
                    "update_performance returned Performance::New unexpectedly"
                ));
            }
        };

        let interval_days = reviewed.interval_days as i64;
        let review_count = reviewed.review_count as i64;
        let review_stage = new_performance.stage();

        sqlx::query!(
            r#"
            UPDATE cards
            SET
                last_reviewed_at = ?,
                stability = ?,
                difficulty = ?,
                interval_raw = ?,
                interval_days = ?,
                due_date = ?,
                review_count = ?,
                review_stage = ?
            WHERE card_hash = ?
            "#,
            reviewed.last_reviewed_at,
            reviewed.stability,
            reviewed.difficulty,
            reviewed.interval_raw,
            interval_days,
            reviewed.due_date,
            review_count,
            review_stage,
            card.card_hash,
        )
        .execute(&self.pool)
        .await?;

        Ok(reviewed.interval_raw)
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
                review_count as "review_count!: i64",
                review_stage as "review_stage!: ReviewStage"
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
        match row.review_stage {
            ReviewStage::LearningA => Ok(Performance::LearningA(reviewed)),
            ReviewStage::LearningB => Ok(Performance::LearningB(reviewed)),
            ReviewStage::Review => Ok(Performance::Review(reviewed)),
            ReviewStage::New => Ok(Performance::New),
        }
    }

    pub async fn due_today(
        &self,
        card_hashes: HashMap<String, Card>,
        card_limit: Option<usize>,
        new_card_limit: Option<usize>,
    ) -> Result<Vec<Card>> {
        let now = (chrono::Utc::now() + LEARN_AHEAD_THRESHOLD_MINS).to_rfc3339();

        // most overdue cards first
        // then cards due today
        // then new cards
        let mut rows = sqlx::query!(
            r#"
        SELECT
            card_hash,
            review_stage as "review_stage!: ReviewStage"
        FROM cards
        WHERE due_date <= ? OR review_stage = ?
        ORDER BY
            CASE WHEN review_stage = ? THEN 1 ELSE 0 END,
            due_date ASC
        "#,
            now,
            ReviewStage::New,
            ReviewStage::New,
        )
        .fetch(&self.pool);

        let mut cards: Vec<Card> = Vec::new();
        let mut num_new_cards = 0;

        while let Some(row) = rows.try_next().await? {
            if !card_hashes.contains_key(&row.card_hash) {
                continue;
            }

            let is_new = row.review_stage == ReviewStage::New;

            if is_new
                && let Some(limit) = new_card_limit
                && num_new_cards >= limit
            {
                continue;
            }

            if let Some(card) = card_hashes.get(&row.card_hash) {
                cards.push(card.clone());

                if is_new {
                    num_new_cards += 1;
                }

                if let Some(limit) = card_limit
                    && cards.len() >= limit
                {
                    break;
                }
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
                due_date as "due_date?: chrono::DateTime<chrono::Utc>",
                interval_raw as "interval_raw?: f64",
                difficulty as "difficulty?: f64",
                stability as "stability?: f64",
                last_reviewed_at as "last_reviewed_at?: chrono::DateTime<chrono::Utc>",
                review_stage as "review_stage!: ReviewStage"
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

#[cfg(test)]
impl DB {
    pub async fn new_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        Self::connect(options).await
    }
}
pub struct CardStatsRow {
    pub card_hash: String,
    pub due_date: Option<chrono::DateTime<chrono::Utc>>,
    pub interval_raw: Option<f64>,
    pub difficulty: Option<f64>,
    pub stability: Option<f64>,
    pub last_reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub review_stage: ReviewStage,
}
