use std::path::Path;

use anyhow::Result;

use anyhow::anyhow;
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use directories::ProjectDirs;

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}

pub fn trim_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn pluralize(word: &str, count: usize) -> String {
    pluralize_with(word, count, |n| n.to_string())
}

pub fn pluralize_with<F>(word: &str, count: usize, format_count: F) -> String
where
    F: Fn(usize) -> String,
{
    let count_str = format_count(count);

    if count == 1 {
        format!("{count_str} {word}")
    } else {
        format!("{count_str} {word}s")
    }
}

pub fn strip_controls_and_escapes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            // ANSI escape sequence (ESC … letter)
            '\x1b' => {
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }

            // Drop all ASCII control characters
            c if c.is_control() => {}

            // Keep everything else (ASCII printable)
            c => out.push(c),
        }
    }

    out.trim().to_string()
}

pub fn ask_yn(prompt: String) -> bool {
    println!("{}", prompt);
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Proceed? ")
        .report(true)
        .wait_for_newline(true)
        .interact()
        .unwrap()
}

pub fn get_data_dir() -> Result<std::path::PathBuf> {
    let proj_dirs = ProjectDirs::from("", "", "repeater")
        .ok_or_else(|| anyhow!("Could not determine project directory"))?;

    let data_dir = proj_dirs.data_dir();
    std::fs::create_dir_all(data_dir)?;

    Ok(data_dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_controls_and_escapes() {
        let input = "\x1b[1mHello\x1b[0m";
        let expected = "Hello";
        assert_eq!(strip_controls_and_escapes(input), expected);
    }
    #[test]
    fn test_is_markdown() {
        assert!(is_markdown(Path::new("test.md")));
        assert!(!is_markdown(Path::new("test.txt")));
    }

    #[test]
    fn test_pluralize_single() {
        assert_eq!(pluralize("card", 1), "1 card");
        assert_eq!(pluralize("cloze card", 1), "1 cloze card");
    }

    #[test]
    fn test_pluralize_multiple() {
        assert_eq!(pluralize("card", 2), "2 cards");
        assert_eq!(pluralize("card", 5), "5 cards");
        assert_eq!(pluralize("cloze card", 3), "3 cloze cards");
    }

    #[test]
    fn test_pluralize_zero() {
        assert_eq!(pluralize("card", 0), "0 cards");
    }
}
