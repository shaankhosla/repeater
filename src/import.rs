use sqlx::Row;
use std::fs::File;
use std::path::Path;
use tempfile::NamedTempFile;
use zip::ZipArchive;

use anyhow::{Context, Result, anyhow};

use crate::crud::DB;

pub async fn run(_db: &DB, anki_path: &Path, export_path: &Path) -> Result<()> {
    validate_paths(anki_path, export_path)?;
    let db_path = extract_collection_db(anki_path)?;
    let db_url = format!("sqlite://{}", db_path.path().display());
    let export_db = sqlx::SqlitePool::connect(&db_url)
        .await
        .context("failed to connect to Anki database")?;
    Ok(())
}

fn validate_paths(anki_path: &Path, export_path: &Path) -> Result<()> {
    if !anki_path.exists() {
        return Err(anyhow!("Anki path does not exist: {}", anki_path.display()));
    }
    if !export_path.exists() {
        return Err(anyhow!(
            "Export path does not exist: {}",
            export_path.display()
        ));
    }
    if !export_path.is_dir() {
        return Err(anyhow!(
            "Export path is not a directory: {}",
            export_path.display()
        ));
    }
    Ok(())
}

fn extract_collection_db(apkg: &Path) -> Result<NamedTempFile> {
    let file = File::open(apkg)
        .with_context(|| format!("failed to open apkg file: {}", apkg.display()))?;

    let mut zip = ZipArchive::new(file).context("failed to read apkg as zip archive")?;

    let mut entry = zip
        .by_name("collection.anki2")
        .context("apkg does not contain collection.anki2")?;

    let mut temp =
        NamedTempFile::new().context("failed to create temporary file for sqlite database")?;

    std::io::copy(&mut entry, &mut temp).context("failed to extract collection.anki2 from apkg")?;

    Ok(temp)
}
