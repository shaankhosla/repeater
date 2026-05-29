use std::path::Path;

use anyhow::Context;
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

pub const DATA_DIR_ENV: &str = "REPEATER_DATA_DIR";

pub fn get_data_dir() -> Result<std::path::PathBuf> {
    if let Ok(override_path) = std::env::var(DATA_DIR_ENV) {
        if override_path.is_empty() {
            return Err(anyhow!(
                "{DATA_DIR_ENV} is set but empty; unset it or point it at a writable directory"
            ));
        }
        let path = std::path::PathBuf::from(override_path);
        std::fs::create_dir_all(&path)
            .with_context(|| format!("failed to create {DATA_DIR_ENV} path {}", path.display()))?;
        return Ok(path);
    }

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

    #[test]
    fn test_get_data_dir_honors_env_override() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("custom-repeater-data");

        let previous = std::env::var(DATA_DIR_ENV).ok();
        unsafe {
            std::env::set_var(DATA_DIR_ENV, &target);
        }

        let resolved = get_data_dir().unwrap();

        unsafe {
            match previous {
                Some(value) => std::env::set_var(DATA_DIR_ENV, value),
                None => std::env::remove_var(DATA_DIR_ENV),
            }
        }

        assert_eq!(resolved, target);
        assert!(target.is_dir(), "override path should be created");
    }

    #[test]
    fn test_get_data_dir_errors_on_empty_env_override() {
        let previous = std::env::var(DATA_DIR_ENV).ok();
        unsafe {
            std::env::set_var(DATA_DIR_ENV, "");
        }

        let resolved = get_data_dir();

        unsafe {
            match previous {
                Some(value) => std::env::set_var(DATA_DIR_ENV, value),
                None => std::env::remove_var(DATA_DIR_ENV),
            }
        }

        let err = resolved.expect_err("empty REPEATER_DATA_DIR must error, not fall back");
        assert!(err.to_string().contains(DATA_DIR_ENV));
    }
}
