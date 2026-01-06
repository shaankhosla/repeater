use super::DB;

use std::collections::HashMap;

use crate::card::Card;

use futures::TryStreamExt;

use crate::stats::CardStats;
use anyhow::Result;

pub struct CardStatsRow {
    pub card_hash: String,
    pub review_count: i64,
    pub due_date: Option<chrono::DateTime<chrono::Utc>>,
    pub interval_raw: Option<f64>,
    pub difficulty: Option<f64>,
    pub stability: Option<f64>,
    pub last_reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl DB {
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
