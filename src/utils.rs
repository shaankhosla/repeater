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

pub fn trim_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn parse_card_lines(contents: &str) -> (Option<String>, Option<String>, Option<String>) {
    #[derive(Copy, Clone)]
    enum Section {
        Question,
        Answer,
        Cloze,
        None,
    }

    let mut question_lines: Vec<&str> = Vec::new();
    let mut answer_lines: Vec<&str> = Vec::new();
    let mut cloze_lines: Vec<&str> = Vec::new();

    let mut section = Section::None;

    for raw_line in contents.lines() {
        let trimmed = trim_line(raw_line);

        if trimmed.is_none() {
            match section {
                Section::Question => question_lines.push(""),
                Section::Answer => answer_lines.push(""),
                Section::Cloze => cloze_lines.push(""),
                Section::None => {}
            }
            continue;
        }

        let line = trimmed.unwrap();
        if line == "---" {
            return (
                join_nonempty(question_lines),
                join_nonempty(answer_lines),
                join_nonempty(cloze_lines),
            );
        }

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
            Section::Question => question_lines.push(line),
            Section::Answer => answer_lines.push(line),
            Section::Cloze => cloze_lines.push(line),
            Section::None => {}
        }
    }

    fn join_nonempty(v: Vec<&str>) -> Option<String> {
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
        Err(anyhow!(
            "Unable to parse anything from card contents:\n{}",
            contents
        ))
    }
}

pub fn get_hash(s: &str) -> Option<String> {
    trim_line(s)?;
    let mut hasher = blake3::Hasher::new();

    // Fast path: pure ASCII (most CLI text tends to be)
    if s.is_ascii() {
        for &b in s.as_bytes() {
            match b {
                b'A'..=b'Z' => {
                    let lower = b + 32;
                    hasher.update(&[lower]);
                }
                b'a'..=b'z' | b'0'..=b'9' | b'+' | b'-' => {
                    hasher.update(&[b]);
                }
                _ => {
                    // drop whitespace, apostrophes, punctuation, etc.
                }
            }
        }
        return Some(hasher.finalize().to_string());
    }

    // Unicode-safe fallback (still streaming; no big allocation)
    let mut buf = [0u8; 4];
    for ch in s.chars() {
        if ch == '+' || ch == '-' {
            hasher.update(&[ch as u8]); // ASCII '+'/'-'
            continue;
        }

        // Keep only letters/digits across Unicode; drop punctuation/whitespace/etc.
        if ch.is_alphanumeric() {
            for lc in ch.to_lowercase() {
                let encoded = lc.encode_utf8(&mut buf);
                hasher.update(encoded.as_bytes());
            }
        }
    }

    Some(hasher.finalize().to_string())
}

pub fn cards_from_md(path: &Path) -> Result<Vec<Card>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let mut cards = Vec::new();
    let mut track_buffer = false;
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
            track_buffer = true;
            if trim_line(&buffer).is_some() {
                cards.push(content_to_card(path, &buffer, start_idx, line_idx)?);
                buffer.clear();
            }
            start_idx = line_idx;
        }
        if line.starts_with("---") && trim_line(&buffer).is_some() {
            cards.push(content_to_card(path, &buffer, start_idx, line_idx)?);
            buffer.clear();
            track_buffer = false;
        }
        if track_buffer {
            buffer.push_str(&line);
        }
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
    use crate::utils::{get_hash, register_all_cards};
    use proptest::prelude::*;
    use std::path::PathBuf;
    proptest! {
        #[test]
        fn test_card_parser( content in "\\PC*") {
            parse_card_lines(&content);
            get_hash(&content);
        }
    }

    #[test]
    fn test_hash() {
        let a = "Hello,  world.\nIt's  2+2 - 1.";
        let b = "hello world its 2+2-1";
        let c = "  HELLO\tWORLD\tIT'S\t2+2 - 1  ";

        let ha = get_hash(a);
        let hb = get_hash(b);
        let hc = get_hash(c);

        assert_eq!(ha, hb);
        assert_eq!(ha, hc);
    }

    #[test]
    fn test_card_parsing() {
        let contents = "C:\nRegion: [`us-east-2`]\n\nLocation: [Ohio]\n\n---\n\n";
        let (question, _, cloze) = parse_card_lines(contents);
        assert!(question.is_none());
        assert_eq!("Region: [`us-east-2`]\n\nLocation: [Ohio]", cloze.unwrap());
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
            "da7c87d9ced65c05181a0cd83c6aa84966b20e6e89f2bff9d9a34927a4c01891"
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
        let db = DB::new_in_memory()
            .await
            .expect("Failed to connect to or initialize database");
        let dir_path = PathBuf::from("test_data");
        let cards = register_all_cards(&db, vec![dir_path]).await.unwrap();
        assert_eq!(cards.len(), 10);
        for card in cards.values() {
            assert!(card.file_path.to_string_lossy().contains("test_data"));
        }

        let dir_path = PathBuf::from("test_data/");
        let file_path = PathBuf::from("test_data/test.md");
        let cards = register_all_cards(&db, vec![dir_path, file_path])
            .await
            .unwrap();
        assert_eq!(cards.len(), 10);
    }

    #[test]
    fn cards_from_md_returns_error_for_nonexistent_file() {
        let path = PathBuf::from("nonexistent_file.md");
        let result = cards_from_md(&path);
        assert!(result.is_err());
    }

    #[test]
    fn content_to_card_returns_error_for_invalid_cloze() {
        let card_path = PathBuf::from("test.md");

        // Cloze without brackets
        let content = "C: this has no cloze markers";
        let result = content_to_card(&card_path, content, 0, 1);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("can't find cloze text")
        );

        // Cloze with empty brackets
        let content = "C: this has empty []";
        let result = content_to_card(&card_path, content, 0, 1);
        assert!(result.is_err());
    }

    #[test]
    fn content_to_card_returns_error_for_incomplete_basic_card() {
        let card_path = PathBuf::from("test.md");

        // Question without answer
        let content = "Q: What is this?\n";
        let result = content_to_card(&card_path, content, 0, 1);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unable to parse anything")
        );

        // Answer without question
        let content = "A: This is an answer\n";
        let result = content_to_card(&card_path, content, 0, 1);
        assert!(result.is_err());
    }

    #[test]
    fn content_to_card_returns_error_for_empty_content() {
        let card_path = PathBuf::from("test.md");
        let result = content_to_card(&card_path, "", 0, 1);
        assert!(result.is_err());
    }

    #[test]
    fn content_to_card_returns_error_for_whitespace_only() {
        let card_path = PathBuf::from("test.md");
        let content = "   \n  \n  ";
        let result = content_to_card(&card_path, content, 0, 1);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn register_all_cards_returns_error_for_invalid_card_file() {
        use std::fs;
        use std::io::Write;

        let db = DB::new_in_memory()
            .await
            .expect("Failed to connect to or initialize database");

        // Create a temporary directory with a malformed markdown file
        let temp_dir = std::env::temp_dir().join("repeat_test_malformed");
        fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("malformed.md");

        // Write malformed card content
        let mut file = fs::File::create(&test_file).unwrap();
        writeln!(file, "Q: This is a question").unwrap();
        writeln!(file, "C: This is invalid [cloze").unwrap(); // Invalid cloze

        let result = register_all_cards(&db, vec![temp_dir.clone()]).await;

        // Clean up
        fs::remove_file(&test_file).unwrap();
        fs::remove_dir(&temp_dir).unwrap();

        // Should return error due to malformed card
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to parse"));
    }
}
