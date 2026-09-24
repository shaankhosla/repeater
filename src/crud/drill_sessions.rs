use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Duration, Utc};
use sqlx::{Row, SqliteConnection};

use crate::fsrs::{ReviewStatus, update_performance};

use super::DB;
use super::cards::{get_card_performance_with, write_card_performance_with};

const SESSION_TTL: Duration = Duration::hours(24);
const SESSION_RETENTION: Duration = Duration::days(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrillSessionSource {
    Paths(Vec<PathBuf>),
    AppleNotes,
}

#[derive(Clone, Debug)]
pub struct NewDrillSession {
    pub session_id: String,
    pub source: DrillSessionSource,
    pub retention: f32,
    pub rephrase_questions: bool,
    pub shuffle: bool,
}

#[derive(Clone, Debug)]
pub struct DrillSession {
    pub session_id: String,
    pub source: DrillSessionSource,
    pub retention: f32,
    pub rephrase_questions: bool,
    pub shuffle: bool,
    pub state: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewDrillReview {
    pub review_id: String,
    pub session_id: String,
    pub card_hash: String,
    pub card_kind: &'static str,
    pub question: String,
    pub answer: String,
    pub source_path: String,
}

#[derive(Clone, Debug)]
pub struct DrillReview {
    pub review_id: String,
    pub session_id: String,
    pub card_hash: String,
    pub card_kind: String,
    pub question: String,
    pub answer: String,
    pub source_path: String,
    pub state: String,
    pub review_result: Option<String>,
    pub interval_raw: Option<f64>,
    pub interval_days: Option<i64>,
    pub due_at: Option<String>,
    pub resulting_review_count: Option<i64>,
}

#[derive(Clone, Debug)]
pub enum MarkReviewResult {
    Applied(DrillReview),
    AlreadyApplied(DrillReview),
    NotFound,
    NotRevealed,
    ConflictingResult,
    SessionExpired,
}

impl DB {
    pub async fn create_drill_session(&self, session: NewDrillSession) -> Result<DrillSession> {
        self.cleanup_drill_sessions().await?;

        let now = Utc::now();
        let expires_at = now + SESSION_TTL;
        let (source_kind, source_paths_json) = match &session.source {
            DrillSessionSource::Paths(paths) => ("paths", serde_json::to_string(paths)?),
            DrillSessionSource::AppleNotes => ("apple_notes", "[]".to_string()),
        };

        sqlx::query(
            r#"
            INSERT INTO drill_sessions (
                session_id,
                source_kind,
                source_paths_json,
                retention,
                rephrase_questions,
                shuffle,
                state,
                created_at,
                last_accessed_at,
                expires_at,
                completed_at
            )
            VALUES (?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, NULL)
            "#,
        )
        .bind(&session.session_id)
        .bind(source_kind)
        .bind(source_paths_json)
        .bind(session.retention)
        .bind(session.rephrase_questions)
        .bind(session.shuffle)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(expires_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(DrillSession {
            session_id: session.session_id,
            source: session.source,
            retention: session.retention,
            rephrase_questions: session.rephrase_questions,
            shuffle: session.shuffle,
            state: "active".to_string(),
            expires_at,
        })
    }

    pub async fn get_drill_session(&self, session_id: &str) -> Result<Option<DrillSession>> {
        let row = sqlx::query(
            r#"
            SELECT
                session_id,
                source_kind,
                source_paths_json,
                retention,
                rephrase_questions,
                shuffle,
                state,
                expires_at
            FROM drill_sessions
            WHERE session_id = ?
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(drill_session_from_row).transpose()
    }

    pub async fn touch_drill_session(&self, session_id: &str) -> Result<()> {
        sqlx::query("UPDATE drill_sessions SET last_accessed_at = ? WHERE session_id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn complete_drill_session(&self, session_id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE drill_sessions SET state = 'complete', completed_at = ?, last_accessed_at = ? WHERE session_id = ? AND state = 'active'",
        )
        .bind(&now)
        .bind(&now)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_open_drill_review(&self, session_id: &str) -> Result<Option<DrillReview>> {
        let row = sqlx::query(
            r#"
            SELECT
                review_id,
                session_id,
                card_hash,
                card_kind,
                question,
                answer,
                source_path,
                state,
                review_result,
                interval_raw,
                interval_days,
                due_at,
                resulting_review_count
            FROM drill_reviews
            WHERE session_id = ? AND state IN ('presented', 'revealed')
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| drill_review_from_row(&row)).transpose()
    }

    pub async fn get_drill_review(&self, review_id: &str) -> Result<Option<DrillReview>> {
        let row = sqlx::query(
            r#"
            SELECT
                review_id,
                session_id,
                card_hash,
                card_kind,
                question,
                answer,
                source_path,
                state,
                review_result,
                interval_raw,
                interval_days,
                due_at,
                resulting_review_count
            FROM drill_reviews
            WHERE review_id = ?
            "#,
        )
        .bind(review_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| drill_review_from_row(&row)).transpose()
    }

    pub async fn create_presented_drill_review(
        &self,
        review: NewDrillReview,
    ) -> Result<DrillReview> {
        let now = Utc::now().to_rfc3339();
        let insert_result = sqlx::query(
            r#"
            INSERT INTO drill_reviews (
                review_id,
                session_id,
                card_hash,
                card_kind,
                question,
                answer,
                source_path,
                state,
                review_result,
                presented_at,
                revealed_at,
                marked_at,
                interval_raw,
                interval_days,
                due_at,
                resulting_review_count
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, 'presented', NULL, ?, NULL, NULL, NULL, NULL, NULL, NULL)
            "#,
        )
        .bind(&review.review_id)
        .bind(&review.session_id)
        .bind(&review.card_hash)
        .bind(review.card_kind)
        .bind(&review.question)
        .bind(&review.answer)
        .bind(&review.source_path)
        .bind(&now)
        .execute(&self.pool)
        .await;

        if let Err(error) = insert_result {
            if let Some(existing) = self.get_open_drill_review(&review.session_id).await? {
                return Ok(existing);
            }
            return Err(error.into());
        }

        self.get_drill_review(&review.review_id)
            .await?
            .ok_or_else(|| anyhow!("created drill review disappeared"))
    }

    pub async fn reveal_drill_review(&self, review_id: &str) -> Result<Option<DrillReview>> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE drill_reviews SET state = 'revealed', revealed_at = ? WHERE review_id = ? AND state = 'presented'",
        )
        .bind(now)
        .bind(review_id)
        .execute(&self.pool)
        .await?;

        self.get_drill_review(review_id).await
    }

    pub async fn mark_drill_review(
        &self,
        review_id: &str,
        review_status: ReviewStatus,
    ) -> Result<MarkReviewResult> {
        let mut connection = self.pool.acquire().await?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *connection)
            .await?;

        let result = mark_drill_review_with(&mut connection, review_id, review_status).await;
        match result {
            Ok(result) => {
                sqlx::query("COMMIT").execute(&mut *connection).await?;
                Ok(result)
            }
            Err(error) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
                Err(error)
            }
        }
    }

    async fn cleanup_drill_sessions(&self) -> Result<()> {
        let now = Utc::now();
        let delete_before = now - SESSION_RETENTION;

        sqlx::query(
            "UPDATE drill_sessions SET state = 'expired' WHERE state = 'active' AND expires_at <= ?",
        )
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "DELETE FROM drill_sessions WHERE state IN ('complete', 'expired') AND last_accessed_at <= ?",
        )
        .bind(delete_before.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

async fn mark_drill_review_with(
    connection: &mut SqliteConnection,
    review_id: &str,
    review_status: ReviewStatus,
) -> Result<MarkReviewResult> {
    let row = sqlx::query(
        r#"
        SELECT
            review_id,
            session_id,
            card_hash,
            card_kind,
            question,
            answer,
            source_path,
            drill_reviews.state,
            review_result,
            interval_raw,
            interval_days,
            due_at,
            resulting_review_count,
            drill_sessions.retention,
            drill_sessions.state AS session_state,
            drill_sessions.expires_at AS session_expires_at
        FROM drill_reviews
        JOIN drill_sessions USING (session_id)
        WHERE review_id = ?
        "#,
    )
    .bind(review_id)
    .fetch_optional(&mut *connection)
    .await?;

    let Some(row) = row else {
        return Ok(MarkReviewResult::NotFound);
    };
    let review = drill_review_from_row(&row)?;
    let requested_result = review_status_name(review_status);
    let session_state: String = row.try_get("session_state")?;
    let session_expires_at =
        DateTime::parse_from_rfc3339(&row.try_get::<String, _>("session_expires_at")?)
            .context("invalid drill session expiration")?
            .with_timezone(&Utc);
    if session_state == "expired" || session_expires_at <= Utc::now() {
        return Ok(MarkReviewResult::SessionExpired);
    }

    if review.state == "marked" {
        return if review.review_result.as_deref() == Some(requested_result) {
            Ok(MarkReviewResult::AlreadyApplied(review))
        } else {
            Ok(MarkReviewResult::ConflictingResult)
        };
    }
    if review.state != "revealed" {
        return Ok(MarkReviewResult::NotRevealed);
    }

    let retention: f32 = row.try_get("retention")?;
    let current_performance = get_card_performance_with(connection, &review.card_hash).await?;
    let new_performance =
        update_performance(current_performance, review_status, Utc::now(), retention)?;
    write_card_performance_with(connection, &review.card_hash, new_performance).await?;

    let interval_days = new_performance.interval_days as i64;
    let review_count = new_performance.review_count as i64;
    let due_at = new_performance.due_date.to_rfc3339();
    let marked_at = new_performance.last_reviewed_at.to_rfc3339();
    sqlx::query(
        r#"
        UPDATE drill_reviews
        SET
            state = 'marked',
            review_result = ?,
            marked_at = ?,
            interval_raw = ?,
            interval_days = ?,
            due_at = ?,
            resulting_review_count = ?
        WHERE review_id = ? AND state = 'revealed'
        "#,
    )
    .bind(requested_result)
    .bind(marked_at)
    .bind(new_performance.interval_raw)
    .bind(interval_days)
    .bind(&due_at)
    .bind(review_count)
    .bind(review_id)
    .execute(&mut *connection)
    .await?;

    let mut marked = review;
    marked.state = "marked".to_string();
    marked.review_result = Some(requested_result.to_string());
    marked.interval_raw = Some(new_performance.interval_raw);
    marked.interval_days = Some(interval_days);
    marked.due_at = Some(due_at);
    marked.resulting_review_count = Some(review_count);
    Ok(MarkReviewResult::Applied(marked))
}

fn drill_session_from_row(row: sqlx::sqlite::SqliteRow) -> Result<DrillSession> {
    let source_kind: String = row.try_get("source_kind")?;
    let source = match source_kind.as_str() {
        "paths" => DrillSessionSource::Paths(serde_json::from_str(
            &row.try_get::<String, _>("source_paths_json")?,
        )?),
        "apple_notes" => DrillSessionSource::AppleNotes,
        value => return Err(anyhow!("unknown drill session source kind {value}")),
    };
    let expires_at = DateTime::parse_from_rfc3339(&row.try_get::<String, _>("expires_at")?)
        .context("invalid drill session expiration")?
        .with_timezone(&Utc);

    Ok(DrillSession {
        session_id: row.try_get("session_id")?,
        source,
        retention: row.try_get("retention")?,
        rephrase_questions: row.try_get("rephrase_questions")?,
        shuffle: row.try_get("shuffle")?,
        state: row.try_get("state")?,
        expires_at,
    })
}

fn drill_review_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<DrillReview> {
    Ok(DrillReview {
        review_id: row.try_get("review_id")?,
        session_id: row.try_get("session_id")?,
        card_hash: row.try_get("card_hash")?,
        card_kind: row.try_get("card_kind")?,
        question: row.try_get("question")?,
        answer: row.try_get("answer")?,
        source_path: row.try_get("source_path")?,
        state: row.try_get("state")?,
        review_result: row.try_get("review_result")?,
        interval_raw: row.try_get("interval_raw")?,
        interval_days: row.try_get("interval_days")?,
        due_at: row.try_get("due_at")?,
        resulting_review_count: row.try_get("resulting_review_count")?,
    })
}

fn review_status_name(status: ReviewStatus) -> &'static str {
    match status {
        ReviewStatus::Pass => "pass",
        ReviewStatus::Fail => "fail",
    }
}
