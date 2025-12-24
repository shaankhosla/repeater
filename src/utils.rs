use ignore::WalkBuilder;
use ignore::types::TypesBuilder;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::card::{Card, CardContent};
use ignore::WalkState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::crud::DB;

use anyhow::{Result, anyhow};

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}

fn find_cloze_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = None;

    for (i, ch) in text.char_indices() {
        match ch {
            '[' if start.is_none() => start = Some(i),
            ']' if start.is_some() => {
                let s = start.take().unwrap();
                let e = i;
                ranges.push((s, e));
            }
            _ => {}
        }
    }

    ranges
}
pub fn trim_line(line: &str) -> Option<String> {
    let trimmed_line = line.trim().to_string();
    if trimmed_line.is_empty() {
        return None;
    }
    Some(trimmed_line)
}

fn parse_card_lines(contents: &str) -> (Option<String>, Option<String>, Option<String>) {
    #[derive(Copy, Clone)]
    enum Section {
        Question,
        Answer,
        Cloze,
        None,
    }

    let mut question_lines = Vec::new();
    let mut answer_lines = Vec::new();
    let mut cloze_lines = Vec::new();

    let mut section = Section::None;

    for raw_line in contents.lines() {
        let trimmed = trim_line(raw_line);

        if trimmed.is_none() {
            match section {
                Section::Question => question_lines.push(String::new()),
                Section::Answer => answer_lines.push(String::new()),
                Section::Cloze => cloze_lines.push(String::new()),
                Section::None => {}
            }
            continue;
        }

        let line = trimmed.unwrap();

        if let Some(rest) = line.strip_prefix("Q:") {
            section = Section::Question;
            question_lines.clear();
            if let Some(v) = trim_line(rest) {
                question_lines.push(v);
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("A:") {
            section = Section::Answer;
            answer_lines.clear();
            if let Some(v) = trim_line(rest) {
                answer_lines.push(v);
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("C:") {
            section = Section::Cloze;
            cloze_lines.clear();
            if let Some(v) = trim_line(rest) {
                cloze_lines.push(v);
            }
            continue;
        }

        match section {
            Section::Question => question_lines.push(line.to_owned()),
            Section::Answer => answer_lines.push(line.to_owned()),
            Section::Cloze => cloze_lines.push(line.to_owned()),
            Section::None => {}
        }
    }

    fn join_nonempty(v: Vec<String>) -> Option<String> {
        if v.is_empty() {
            return None;
        }

        let total_len: usize = v.iter().map(|s| s.len()).sum::<usize>() + v.len().saturating_sub(1);
        let mut out = String::with_capacity(total_len);

        for (i, line) in v.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(line);
        }

        if out.trim().is_empty() {
            None
        } else {
            while out.ends_with(char::is_whitespace) {
                out.pop();
            }
            Some(out)
        }
    }

    (
        join_nonempty(question_lines),
        join_nonempty(answer_lines),
        join_nonempty(cloze_lines),
    )
}
pub fn content_to_card(
    card_path: &Path,
    contents: &str,
    file_start_idx: usize,
    file_end_idx: usize,
) -> Result<Card> {
    let (question, answer, cloze) = parse_card_lines(contents);

    let card_hash = get_hash(contents).ok_or_else(|| anyhow!("Unable to hash contents"))?;
    if let (Some(q), Some(a)) = (question, answer) {
        let content = CardContent::Basic {
            question: q,
            answer: a,
        };
        Ok(Card {
            file_path: card_path.to_path_buf(),
            file_card_range: (file_start_idx, file_end_idx),
            content,
            card_hash,
        })
    } else if let Some(c) = cloze {
        let cloze_idxs = find_cloze_ranges(&c);
        if cloze_idxs.is_empty() {
            return Err(anyhow!("Card is a cloze but can't find cloze text in []"));
        }
        let cloze_idx_start = cloze_idxs[0].0;
        let cloze_idx_end = cloze_idxs[0].1;
        if cloze_idx_end - cloze_idx_start <= 1 {
            return Err(anyhow!("Card is a cloze but can't find cloze text in []"));
        }
        let content = CardContent::Cloze {
            text: c,
            start: cloze_idx_start,
            end: cloze_idx_end,
        };
        Ok(Card {
            file_path: card_path.to_path_buf(),
            file_card_range: (file_start_idx, file_end_idx),
            content,
            card_hash,
        })
    } else {
        Err(anyhow!("Unable to create card: {}", card_path.display()))
    }
}

pub fn get_hash(content: &str) -> Option<String> {
    if let Some(content) = trim_line(content) {
        return Some(blake3::hash(content.as_bytes()).to_string());
    }
    None
}

pub fn cards_from_md(path: &Path) -> Result<Vec<Card>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let mut cards = Vec::new();
    let mut buffer = String::new();
    let mut line = String::new();
    let mut start_idx = 0;
    let mut last_idx = 0;
    let mut line_idx = 0;

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        if line.starts_with("Q:") || line.starts_with("C:") {
            if !buffer.is_empty() {
                cards.push(content_to_card(path, &buffer, start_idx, line_idx)?);
                buffer.clear();
            }
            start_idx = line_idx;
        }
        buffer.push_str(&line);
        last_idx = line_idx;
        line_idx += 1;
    }
    if !buffer.is_empty() {
        cards.push(content_to_card(path, &buffer, start_idx, last_idx + 1)?);
    }

    Ok(cards)
}

fn markdown_walk_builder(paths: &[PathBuf]) -> Result<Option<WalkBuilder>> {
    let mut iter = paths.iter();
    let Some(first) = iter.next() else {
        return Ok(None);
    };
    let mut builder = WalkBuilder::new(first);
    for path in iter {
        builder.add(path);
    }
    builder.hidden(false).git_ignore(true).git_exclude(true);
    let mut types = TypesBuilder::new();
    types.add("markdown", "*.md")?;
    types.select("markdown");
    builder.types(types.build()?);
    Ok(Some(builder))
}

fn run_card_walker(paths: Vec<PathBuf>, sender: mpsc::UnboundedSender<Vec<Card>>) -> Result<()> {
    let Some(builder) = markdown_walk_builder(&paths)? else {
        return Ok(());
    };

    let error_slot = Arc::new(Mutex::new(None));

    builder.build_parallel().run(|| {
        let sender = sender.clone();
        let error_slot = Arc::clone(&error_slot);
        Box::new(move |entry| match entry {
            Ok(entry) => {
                if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    return WalkState::Continue;
                }
                let path = entry.path().to_path_buf();
                match cards_from_md(&path) {
                    Ok(cards) => {
                        if cards.is_empty() {
                            return WalkState::Continue;
                        }
                        if sender.send(cards).is_err() {
                            return WalkState::Quit;
                        }
                    }
                    Err(err) => {
                        *error_slot.lock().unwrap() =
                            Some(err.context(format!("Failed to parse {}", path.display())));
                        return WalkState::Quit;
                    }
                }
                WalkState::Continue
            }
            Err(err) => {
                *error_slot.lock().unwrap() = Some(anyhow!(err));
                WalkState::Quit
            }
        })
    });

    drop(sender);

    if let Some(err) = error_slot.lock().unwrap().take() {
        return Err(err);
    }
    Ok(())
}

pub async fn register_all_cards(db: &DB, paths: Vec<PathBuf>) -> Result<HashMap<String, Card>> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<Card>>();
    let walker_handle = tokio::task::spawn_blocking(move || run_card_walker(paths, tx));

    let mut hash_cards = HashMap::new();
    while let Some(batch) = rx.recv().await {
        if batch.is_empty() {
            continue;
        }
        db.add_cards_batch(&batch).await?;
        for card in batch {
            hash_cards.insert(card.card_hash.clone(), card);
        }
    }

    walker_handle.await??;

    Ok(hash_cards)
}

#[cfg(test)]
mod tests {
    use super::{cards_from_md, content_to_card, parse_card_lines};
    use crate::card::CardContent;
    use crate::crud::DB;
    use crate::utils::register_all_cards;
    use std::path::PathBuf;

    #[test]
    fn test_card_parsing() {
        let contents = "C:\nRegion: [`us-east-2`]\n\nLocation: [Ohio]\n\n---\n\n";
        let (question, _, cloze) = parse_card_lines(contents);
        assert!(question.is_none());
        assert_eq!(
            "Region: [`us-east-2`]\n\nLocation: [Ohio]\n\n---",
            cloze.unwrap()
        );
    }

    #[test]
    fn basic_qa() {
        let card_path = PathBuf::from("test.md");

        let card = content_to_card(&card_path, "", 1, 1);
        assert!(card.is_err());

        let card = content_to_card(&card_path, "what am i doing here", 1, 1);
        assert!(card.is_err());

        let content = "Q: what?\nA: yes\n\n";
        let card = content_to_card(&card_path, content, 1, 1).unwrap();
        assert_eq!(
            card.card_hash,
            "a3d83e3e6aa97dad07e955c6bc819baf8ff654dc086bc12fbb1dacc1a92f8e5e"
        );
        if let CardContent::Basic { question, answer } = &card.content {
            assert_eq!(question, "what?");
            assert_eq!(answer, "yes");
        } else {
            panic!("Expected CardContent::Basic");
        }

        let content = "Q: what?\nA: \n\n";
        let card = content_to_card(&card_path, content, 1, 1);
        assert!(card.is_err());
    }

    #[test]
    fn basic_cloze() {
        let card_path = PathBuf::from("test.md");

        let content = "C: ping? [pong]";
        let card = content_to_card(&card_path, content, 1, 1);
        if let CardContent::Cloze { text, start, end } = &card.expect("should be basic").content {
            assert_eq!(text, "ping? [pong]");
            assert_eq!(*start, 6_usize);
            assert_eq!(*end, 11_usize);
        } else {
            panic!("Expected CardContent::Cloze");
        }
    }

    #[test]
    fn test_file_capture() {
        let card_path = PathBuf::from("test_data/test.md");
        let cards = cards_from_md(&card_path).expect("should be ok");

        assert_eq!(cards.len(), 8);
    }

    #[tokio::test]
    async fn collects_cards_from_directory() {
        let db = DB::new()
            .await
            .expect("Failed to connect to or initialize database");
        let dir_path = PathBuf::from("test_data");
        let cards = register_all_cards(&db, vec![dir_path]).await.unwrap();
        assert_eq!(cards.len(), 8);
        for card in cards.values() {
            assert!(card.file_path.ends_with("test_data/test.md"));
        }

        let dir_path = PathBuf::from("test_data/");
        let file_path = PathBuf::from("test_data/test.md");
        let cards = register_all_cards(&db, vec![dir_path, file_path])
            .await
            .unwrap();
        assert_eq!(cards.len(), 8);
    }
}
