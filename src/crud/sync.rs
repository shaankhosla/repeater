use anyhow::Result;
use sqlx::{Row, SqliteExecutor};

use crate::sync::types::{SyncCard, remote_wins};

use super::DB;

impl DB {
    pub async fn get_locally_modified_cards(&self) -> Result<Vec<SyncCard>> {
        let rows = sqlx::query(
            "SELECT card_hash, last_reviewed_at, stability, difficulty, interval_raw, \
             interval_days, due_date, review_count, added_at \
             FROM cards WHERE locally_modified = 1",
        )
        .fetch_all(&self.pool)
        .await?;

        let cards = rows
            .into_iter()
            .map(|row| SyncCard {
                card_hash: row.get("card_hash"),
                last_reviewed_at: row.get("last_reviewed_at"),
                stability: row.get("stability"),
                difficulty: row.get("difficulty"),
                interval_raw: row.get("interval_raw"),
                interval_days: row.get::<Option<i32>, _>("interval_days").map(|v| v as i64),
                due_date: row.get("due_date"),
                review_count: row.get::<i32, _>("review_count") as i64,
                added_at: row.get("added_at"),
            })
            .collect();

        Ok(cards)
    }

    pub async fn clear_locally_modified(&self) -> Result<()> {
        sqlx::query("UPDATE cards SET locally_modified = 0 WHERE locally_modified = 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_locally_modified(&self, card_hash: &str) -> Result<()> {
        sqlx::query("UPDATE cards SET locally_modified = 1 WHERE card_hash = ?1")
            .bind(card_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Merge cards pulled from the server. Uses last-write-wins based on last_reviewed_at.
    pub async fn merge_pulled_cards(&self, cards: &[SyncCard]) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for card in cards {
            let local = sqlx::query(
                "SELECT last_reviewed_at, review_count FROM cards WHERE card_hash = ?1",
            )
            .bind(&card.card_hash)
            .fetch_optional(&mut *tx)
            .await?;

            match local {
                Some(local_row) => {
                    let local_reviewed_at: Option<String> = local_row.get("last_reviewed_at");

                    if remote_wins(&card.last_reviewed_at, &local_reviewed_at) {
                        update_card_from_sync(&mut *tx, card).await?;
                    }
                }
                None => {
                    insert_card_from_sync(&mut *tx, card).await?;
                }
            }
        }

        tx.commit().await?;
        Ok(())
    }
}

async fn update_card_from_sync<'e, E: SqliteExecutor<'e>>(
    executor: E,
    card: &SyncCard,
) -> Result<()> {
    sqlx::query(
        "UPDATE cards SET last_reviewed_at = ?1, stability = ?2, difficulty = ?3, \
         interval_raw = ?4, interval_days = ?5, due_date = ?6, review_count = ?7 \
         WHERE card_hash = ?8",
    )
    .bind(&card.last_reviewed_at)
    .bind(card.stability)
    .bind(card.difficulty)
    .bind(card.interval_raw)
    .bind(card.interval_days.map(|v| v as i32))
    .bind(&card.due_date)
    .bind(card.review_count as i32)
    .bind(&card.card_hash)
    .execute(executor)
    .await?;
    Ok(())
}

async fn insert_card_from_sync<'e, E: SqliteExecutor<'e>>(
    executor: E,
    card: &SyncCard,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO cards (card_hash, added_at, last_reviewed_at, stability, difficulty, \
         interval_raw, interval_days, due_date, review_count, locally_modified) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)",
    )
    .bind(&card.card_hash)
    .bind(&card.added_at)
    .bind(&card.last_reviewed_at)
    .bind(card.stability)
    .bind(card.difficulty)
    .bind(card.interval_raw)
    .bind(card.interval_days.map(|v| v as i32))
    .bind(&card.due_date)
    .bind(card.review_count as i32)
    .execute(executor)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::fsrs::ReviewStatus;
    use crate::parser::content_to_card;
    use crate::sync::types::SyncCard;

    use super::DB;

    fn make_sync_card(hash: &str, reviewed_at: Option<&str>, review_count: i64) -> SyncCard {
        SyncCard {
            card_hash: hash.to_string(),
            last_reviewed_at: reviewed_at.map(|s| s.to_string()),
            stability: Some(5.0),
            difficulty: Some(5.0),
            interval_raw: Some(1.0),
            interval_days: Some(1),
            due_date: Some("2026-04-01T00:00:00+00:00".to_string()),
            review_count,
            added_at: "2026-03-29T00:00:00+00:00".to_string(),
        }
    }

    #[tokio::test]
    async fn merge_inserts_new_card() {
        let db = DB::new_in_memory().await.unwrap();
        let remote = make_sync_card("hash_new", Some("2026-03-29T12:00:00+00:00"), 3);

        db.merge_pulled_cards(&[remote]).await.unwrap();

        let modified = db.get_locally_modified_cards().await.unwrap();
        assert!(modified.is_empty(), "inserted-from-sync cards should not be locally_modified");

        let row = sqlx::query_scalar::<_, i32>(
            "SELECT review_count FROM cards WHERE card_hash = 'hash_new'",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(row, 3);
    }

    #[tokio::test]
    async fn merge_remote_wins_when_newer() {
        let db = DB::new_in_memory().await.unwrap();
        let card = content_to_card(&PathBuf::from("t.md"), "C: ping? [pong]", 1, 1).unwrap();
        db.add_card(&card).await.unwrap();
        db.update_card_performance(&card, ReviewStatus::Pass, None, 0.9).await.unwrap();

        let remote = make_sync_card(&card.card_hash, Some("2099-01-01T00:00:00+00:00"), 99);
        db.merge_pulled_cards(&[remote]).await.unwrap();

        let row = sqlx::query_scalar::<_, i32>(
            "SELECT review_count FROM cards WHERE card_hash = ?1",
        )
        .bind(&card.card_hash)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(row, 99, "remote with newer timestamp should win");
    }

    #[tokio::test]
    async fn merge_local_wins_when_newer() {
        let db = DB::new_in_memory().await.unwrap();
        let card = content_to_card(&PathBuf::from("t.md"), "C: ping? [pong]", 1, 1).unwrap();
        db.add_card(&card).await.unwrap();
        db.update_card_performance(&card, ReviewStatus::Pass, None, 0.9).await.unwrap();

        let remote = make_sync_card(&card.card_hash, Some("2000-01-01T00:00:00+00:00"), 99);
        db.merge_pulled_cards(&[remote]).await.unwrap();

        let row = sqlx::query_scalar::<_, i32>(
            "SELECT review_count FROM cards WHERE card_hash = ?1",
        )
        .bind(&card.card_hash)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(row, 1, "local with newer timestamp should win");
    }

    #[tokio::test]
    async fn merge_both_none_is_noop() {
        let db = DB::new_in_memory().await.unwrap();
        let card = content_to_card(&PathBuf::from("t.md"), "C: ping? [pong]", 1, 1).unwrap();
        db.add_card(&card).await.unwrap();

        let remote = make_sync_card(&card.card_hash, None, 99);
        db.merge_pulled_cards(&[remote]).await.unwrap();

        let row = sqlx::query_scalar::<_, i32>(
            "SELECT review_count FROM cards WHERE card_hash = ?1",
        )
        .bind(&card.card_hash)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(row, 0, "both None should not update");
    }

    #[tokio::test]
    async fn update_card_performance_sets_locally_modified() {
        let db = DB::new_in_memory().await.unwrap();
        let card = content_to_card(&PathBuf::from("t.md"), "C: ping? [pong]", 1, 1).unwrap();
        db.add_card(&card).await.unwrap();

        let modified = db.get_locally_modified_cards().await.unwrap();
        assert!(modified.is_empty(), "new card should not be locally_modified");

        db.update_card_performance(&card, ReviewStatus::Pass, None, 0.9).await.unwrap();

        let modified = db.get_locally_modified_cards().await.unwrap();
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0].card_hash, card.card_hash);
    }

    #[tokio::test]
    async fn clear_locally_modified_only_clears_flagged() {
        let db = DB::new_in_memory().await.unwrap();
        let c1 = content_to_card(&PathBuf::from("t.md"), "C: one? [1]", 1, 1).unwrap();
        let c2 = content_to_card(&PathBuf::from("t.md"), "C: two? [2]", 2, 2).unwrap();
        db.add_card(&c1).await.unwrap();
        db.add_card(&c2).await.unwrap();
        db.update_card_performance(&c1, ReviewStatus::Pass, None, 0.9).await.unwrap();

        db.clear_locally_modified().await.unwrap();

        db.update_card_performance(&c2, ReviewStatus::Pass, None, 0.9).await.unwrap();

        let modified = db.get_locally_modified_cards().await.unwrap();
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0].card_hash, c2.card_hash);
    }
}
