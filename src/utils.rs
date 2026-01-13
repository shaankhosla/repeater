use std::path::Path;

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
}
