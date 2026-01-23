use anyhow::Result;

use crate::check_version::VersionUpdateStats;

use super::DB;

impl DB {
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
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_version() {
        let db = DB::new_in_memory().await.unwrap();
        let version = db.get_version_update_information().await.unwrap();
        assert!(version.last_version_check_at.is_none());
        assert!(version.last_prompted_at.is_none());

        db.update_last_version_check_at().await.unwrap();
        let version = db.get_version_update_information().await.unwrap();
        assert!(version.last_version_check_at.is_some());
        assert!(version.last_prompted_at.is_none());

        db.update_last_prompted_at().await.unwrap();
        let version = db.get_version_update_information().await.unwrap();
        assert!(version.last_version_check_at.is_some());
        assert!(version.last_prompted_at.is_some());
    }
}
