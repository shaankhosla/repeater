use html_escape::decode_html_entities;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use zip::ZipArchive;

use anyhow::{Context, Result, anyhow};

use crate::crud::DB;

static TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<[^>]+>").unwrap());
static CLOZE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?s)\{\{c\d+::(.*?)(?:::(.*?))?\}\}").unwrap());

#[derive(Clone)]
struct DeckInfo {
    name: String,
    components: Vec<String>,
}

#[derive(Clone, Copy)]
enum ModelKind {
    Basic,
    Cloze,
}

#[derive(Clone, Debug)]
struct CardRecord {
    deck_id: i64,
    model_id: i64,
    ord: i64,
    fields: Vec<String>,
}

pub async fn run(_db: &DB, anki_path: &Path, export_path: &Path) -> Result<()> {
    validate_paths(anki_path, export_path)?;
    let db_path = extract_collection_db(anki_path)?;
    let db_url = format!("sqlite://{}", db_path.path().display());
    let export_db = SqlitePool::connect(&db_url)
        .await
        .context("failed to connect to Anki database")?;
    let (decks, models) = load_metadata(&export_db).await?;
    let cards = load_cards(&export_db).await?;
    let exports = build_exports(cards, &models);
    write_exports(export_path, &decks, exports)?;
    Ok(())
}

fn validate_paths(anki_path: &Path, export_path: &Path) -> Result<()> {
    if !anki_path.exists() {
        return Err(anyhow!("Anki path does not exist: {}", anki_path.display()));
    }
    if !anki_path.is_file() || anki_path.extension() != Some("apkg".as_ref()) {
        return Err(anyhow!(
            "Anki path does not point to an apkg file: {}",
            anki_path.display()
        ));
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
        .by_name("collection.anki21")
        .context("apkg does not contain collection.anki21, the newer export format DB")?;

    let mut temp =
        NamedTempFile::new().context("failed to create temporary file for sqlite database")?;

    std::io::copy(&mut entry, &mut temp).context("failed to extract collection.anki2 from apkg")?;

    Ok(temp)
}

async fn load_metadata(
    pool: &SqlitePool,
) -> Result<(HashMap<i64, DeckInfo>, HashMap<i64, ModelKind>)> {
    let row = sqlx::query("SELECT decks, models FROM col LIMIT 1")
        .fetch_one(pool)
        .await
        .context("failed to read deck metadata")?;
    let decks_raw: String = row.try_get("decks")?;
    let models_raw: String = row.try_get("models")?;
    let decks = parse_decks(&decks_raw)?;
    let models = parse_models(&models_raw)?;
    Ok((decks, models))
}

fn parse_decks(json: &str) -> Result<HashMap<i64, DeckInfo>> {
    let value: Value = serde_json::from_str(json).context("failed to parse decks json")?;
    let mut decks = HashMap::new();
    if let Some(map) = value.as_object() {
        for deck in map.values() {
            if let Some(id) = deck.get("id").and_then(|v| v.as_i64()) {
                // name could be Data Science::clustering
                let name = deck.get("name").and_then(|v| v.as_str()).unwrap_or("Deck");
                decks.insert(
                    id,
                    DeckInfo {
                        name: name.to_string(),
                        components: deck_components(name),
                    },
                );
            }
        }
    }
    Ok(decks)
}

fn parse_models(json: &str) -> Result<HashMap<i64, ModelKind>> {
    let value: Value = serde_json::from_str(json).context("failed to parse models json")?;
    let mut models = HashMap::new();
    if let Some(map) = value.as_object() {
        for model in map.values() {
            if let Some(id) = model.get("id").and_then(|v| v.as_i64()) {
                let kind = match model.get("type").and_then(|v| v.as_i64()).unwrap_or(0) {
                    1 => ModelKind::Cloze,
                    _ => ModelKind::Basic,
                };
                models.insert(id, kind);
            }
        }
    }
    Ok(models)
}

async fn load_cards(pool: &SqlitePool) -> Result<Vec<CardRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT
            cards.did as did,
            cards.ord as ord,
            notes.mid as mid,
            notes.flds as flds
        FROM cards
        JOIN notes ON notes.id = cards.nid
        ORDER BY cards.did, notes.id, cards.ord
        "#,
    )
    .fetch_all(pool)
    .await?;
    let mut cards = Vec::with_capacity(rows.len());
    for row in rows {
        let deck_id: i64 = row.try_get("did")?;
        let ord: i64 = row.try_get("ord")?;
        let model_id: i64 = row.try_get("mid")?;

        //"Examples of supervised methods with built-in feature selection\u{1f}Decision trees<br><div>LASSO (linear regression with L1 regularization)</div>\u{1f}<a href=\"https://machinelearningmastery.com/feature-selection-with-real-and-categorical-data/\">https://machinelearningmastery.com/feature-selection-with-real-and-categorical-data/</a>\u{1f}"
        let fields_raw: String = row.try_get("flds")?;
        let card = CardRecord {
            deck_id,
            model_id,
            ord,
            fields: split_fields(&fields_raw),
        };
        cards.push(card);
    }
    Ok(cards)
}

fn build_exports(
    cards: Vec<CardRecord>,
    models: &HashMap<i64, ModelKind>,
) -> HashMap<i64, Vec<String>> {
    let mut per_deck: HashMap<i64, Vec<String>> = HashMap::new();
    for card in cards {
        let Some(model) = models.get(&card.model_id) else {
            continue;
        };
        let entry = match model {
            ModelKind::Basic => basic_entry(&card.fields, card.ord),
            ModelKind::Cloze => cloze_entry(&card.fields),
        };
        if let Some(content) = entry {
            per_deck.entry(card.deck_id).or_default().push(content);
        }
    }
    per_deck
}

fn write_exports(
    export_path: &Path,
    decks: &HashMap<i64, DeckInfo>,
    exports: HashMap<i64, Vec<String>>,
) -> Result<()> {
    let mut entries: Vec<(i64, Vec<String>)> = exports
        .into_iter()
        .filter(|(_, cards)| !cards.is_empty())
        .collect();
    entries.sort_by(|(a, _), (b, _)| {
        let name_a = decks.get(a).map(|d| d.name.as_str()).unwrap_or("");
        let name_b = decks.get(b).map(|d| d.name.as_str()).unwrap_or("");
        name_a.cmp(name_b)
    });
    for (deck_id, cards) in entries {
        let deck = decks
            .get(&deck_id)
            .ok_or_else(|| anyhow!("missing deck metadata for id {}", deck_id))?;
        let mut path = PathBuf::from(export_path);
        if deck.components.len() > 1 {
            for component in &deck.components[..deck.components.len() - 1] {
                path.push(component);
            }
        }
        let file_stem = deck
            .components
            .last()
            .cloned()
            .unwrap_or_else(|| "Deck".to_string());
        path.push(format!("{file_stem}.md"));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut content = String::new();
        for card in cards {
            content.push_str(&card);
        }
        fs::write(&path, content)?;
    }
    Ok(())
}

fn split_fields(raw: &str) -> Vec<String> {
    raw.split('\x1f').map(clean_field).collect()
}

fn clean_field(field: &str) -> String {
    let mut text = field.replace("\r\n", "\n");
    text = text.replace("<br />", "\n");
    text = text.replace("<br>", "\n");
    text = text.replace("<div>", "\n");
    text = text.replace("</div>", "\n");
    text = text.replace("<p>", "\n");
    text = text.replace("</p>", "\n");
    text = text.replace("<li>", "\n- ");
    text = text.replace("</li>", "");
    let without_tags = TAG_RE.replace_all(&text, "");
    decode_html_entities(without_tags.trim()).to_string()
}

fn deck_components(name: &str) -> Vec<String> {
    let mut parts: Vec<String> = name
        .split("::")
        .map(sanitize_component)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        parts.push("Deck".to_string());
    }
    parts
}

fn sanitize_component(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            out.push('-');
        } else {
            out.push(ch);
        }
    }
    out.trim().to_string()
}

fn basic_entry(fields: &[String], ord: i64) -> Option<String> {
    if fields.len() < 2 {
        return None;
    }
    let (question, answer) = if ord % 2 == 0 {
        (&fields[0], &fields[1])
    } else {
        (&fields[1], &fields[0])
    };
    let mut entry = format_section("Q", question)?;
    entry.push_str(&format_section("A", answer)?);
    entry.push('\n');
    Some(entry)
}

fn cloze_entry(fields: &[String]) -> Option<String> {
    let text = fields.first()?;
    let converted = convert_cloze(text);
    let mut entry = format_section("C", converted.trim())?;
    entry.push('\n');
    Some(entry)
}

fn format_section(label: &str, value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut out = String::new();
    out.push_str(label);
    out.push_str(": ");
    out.push_str(trimmed);
    out.push('\n');
    Some(out)
}

fn convert_cloze(text: &str) -> String {
    CLOZE_RE
        .replace_all(text, |caps: &regex::Captures| {
            let inner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            format!("[{}]", inner.trim())
        })
        .into_owned()
}
