use anyhow::Result;
use futures::TryStreamExt;

use std::collections::HashMap;

use anyhow::anyhow;

use crate::card::Card;

use crate::fsrs::ReviewStatus;
use crate::fsrs::ReviewedPerformance;
use crate::fsrs::update_performance;
use crate::fsrs::{LEARN_AHEAD_THRESHOLD_MINS, Performance};

use super::DB;

impl DB {
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
        optional_now: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<f64> {
        let current_performance = self.get_card_performance(card).await?;
        let now = match optional_now {
            Some(now) => now,
            None => chrono::Utc::now(),
        };

        let new_performance = update_performance(current_performance, review_status, now)?;

        let interval_days = new_performance.interval_days as i64;
        let review_count = new_performance.review_count as i64;

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

        Ok(new_performance.interval_raw)
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
        card_hashes: &HashMap<String, Card>,
        card_limit: Option<usize>,
        new_card_limit: Option<usize>,
    ) -> Result<Vec<Card>> {
        let now = (chrono::Utc::now() + LEARN_AHEAD_THRESHOLD_MINS).to_rfc3339();

        // most overdue cards first
        // then cards due today
        // then new cards
        let mut rows = sqlx::query!(
            r#"
        SELECT card_hash, review_count as "review_count!: i64"
        FROM cards
        WHERE due_date <= ? OR due_date IS NULL
        ORDER BY
            CASE WHEN due_date IS NULL THEN 1 ELSE 0 END,
            due_date ASC
        "#,
            now
        )
        .fetch(&self.pool);

        let mut cards: Vec<Card> = Vec::new();
        let mut num_new_cards = 0;

        while let Some(row) = rows.try_next().await? {
            if !card_hashes.contains_key(&row.card_hash) {
                continue;
            }

            let is_new = row.review_count == 0;

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
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::fsrs::{Performance, ReviewStatus};
    use crate::parser::content_to_card;
    use crate::stats::CardLifeCycle;

    use super::DB;

    #[tokio::test]
    async fn follow_card_progress() {
        let content = "C: ping? [pong]";
        let card_path = PathBuf::from("test.md");

        // add card
        let db = DB::new_in_memory().await.unwrap();
        let card = content_to_card(&card_path, content, 1, 1).unwrap();
        db.add_card(&card.clone()).await.unwrap();

        // should exist
        assert!(db.card_exists(&card).await.unwrap());

        // should be in stats
        let card_hashes = HashMap::from([(card.card_hash.clone(), card.clone())]);
        let stats = db.collection_stats(&card_hashes).await.unwrap();
        assert_eq!(stats.num_cards, 1);
        assert_eq!(stats.due_cards, 1);
        assert_eq!(stats.card_lifecycles.get(&CardLifeCycle::New).unwrap(), &1);

        // should be due today
        let due_today_cards = db.due_today(&card_hashes, None, None).await.unwrap();
        assert_eq!(due_today_cards.len(), 1);

        // check short-term scheduling
        for _ in 0..3 {
            db.update_card_performance(&card, ReviewStatus::Pass, None)
                .await
                .unwrap();
        }

        match db.get_card_performance(&card).await.unwrap() {
            Performance::Reviewed(reviewed) => {
                assert_eq!(reviewed.review_count, 3);
                assert_eq!(reviewed.interval_raw, 1.0);
            }
            _ => panic!(),
        }

        // wait the interval and then pass again
        let mut future_time = chrono::Utc::now() + chrono::Duration::days(1);
        db.update_card_performance(&card, ReviewStatus::Pass, Some(future_time))
            .await
            .unwrap();

        match db.get_card_performance(&card).await.unwrap() {
            Performance::Reviewed(reviewed) => {
                assert_eq!(reviewed.review_count, 4);
                assert_eq!(reviewed.interval_raw, 7.0);
            }
            _ => panic!(),
        }

        // now collapse it with a failure
        future_time += chrono::Duration::days(7);
        db.update_card_performance(&card, ReviewStatus::Fail, Some(future_time))
            .await
            .unwrap();

        match db.get_card_performance(&card).await.unwrap() {
            Performance::Reviewed(reviewed) => {
                assert_eq!(reviewed.review_count, 5);
                assert_eq!(reviewed.interval_raw, 1.0);
            }
            _ => panic!(),
        }

        // another failure
        future_time += chrono::Duration::days(2);
        db.update_card_performance(&card, ReviewStatus::Fail, Some(future_time))
            .await
            .unwrap();

        match db.get_card_performance(&card).await.unwrap() {
            Performance::Reviewed(reviewed) => {
                assert_eq!(reviewed.review_count, 6);
                assert_eq!(reviewed.interval_raw, 1.0);
            }
            _ => panic!(),
        }
    }
}
