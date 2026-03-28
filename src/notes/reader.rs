use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use sqlx::Row;
use sqlx::sqlite::SqliteConnectOptions;

use crate::card::Card;
use crate::crud::DB;
use crate::parser::{FileSearchStats, cards_from_text};

use super::converter::decode_note_data;

fn notes_db_path() -> Result<PathBuf> {
    let home = dirs_path()?;
    let path = home
        .join("Library")
        .join("Group Containers")
        .join("group.com.apple.notes")
        .join("NoteStore.sqlite");

    if !path.exists() {
        bail!(
            "Apple Notes database not found at {}. This feature requires macOS with Apple Notes.",
            path.display()
        );
    }

    Ok(path)
}

fn dirs_path() -> Result<PathBuf> {
    directories::UserDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .context("Could not determine home directory")
}

pub async fn register_apple_notes_cards(
    db: &DB,
) -> Result<(HashMap<String, Card>, FileSearchStats)> {
    let db_path = notes_db_path()?;

    let options = SqliteConnectOptions::new()
        .filename(&db_path)
        .read_only(true);

    let pool = sqlx::SqlitePool::connect_with(options)
        .await
        .context("Failed to open Apple Notes database. Make sure Apple Notes has been used at least once and that your terminal has Full Disk Access (System Settings > Privacy & Security > Full Disk Access).")?;

    let rows = sqlx::query(
        "SELECT n.ZTITLE1 as title, note_data.ZDATA as data \
         FROM ZICCLOUDSYNCINGOBJECT n \
         JOIN ZICNOTEDATA note_data ON n.ZNOTEDATA = note_data.Z_PK \
         LEFT JOIN ZICCLOUDSYNCINGOBJECT f ON n.ZFOLDER = f.Z_PK \
         WHERE n.Z_ENT = 12 \
         AND n.ZMARKEDFORDELETION != 1 \
         AND (f.ZTITLE2 IS NULL OR f.ZTITLE2 != 'Recently Deleted')",
    )
    .fetch_all(&pool)
    .await
    .context("Failed to query Apple Notes database")?;

    pool.close().await;

    let mut hash_cards = HashMap::new();
    let mut notes_processed = 0;

    for row in &rows {
        let title: &str = row.try_get("title").unwrap_or("Untitled");
        let data: Option<Vec<u8>> = row.try_get("data").ok();

        let Some(data) = data else {
            continue;
        };

        if data.is_empty() {
            continue;
        }

        let text = match decode_note_data(&data) {
            Ok(text) => text,
            Err(_) => continue,
        };

        if text.is_empty() {
            continue;
        }

        let virtual_path = PathBuf::from(format!("apple-notes://{}", title));
        let cards = match cards_from_text(&virtual_path, &text) {
            Ok(cards) => cards,
            Err(_) => continue,
        };

        if cards.is_empty() {
            continue;
        }

        notes_processed += 1;
        db.add_cards_batch(&cards).await?;
        for card in cards {
            hash_cards.insert(card.card_hash.clone(), card);
        }
    }

    let stats = FileSearchStats {
        files_searched: rows.len(),
        markdown_files: notes_processed,
    };

    Ok((hash_cards, stats))
}
