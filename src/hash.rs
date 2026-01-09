use blake3::Hasher;

// things that shouldn't change hash
// Leading/trailing whitespace
// Multiple spaces / tabs / newlines
// Line wrapping differences
// Case differences

// things that should change hash
// Word order changes
// Punctuation changes
// Stopwords
// Hyphens vs spaces
// Symbols
// Anything semantic

pub fn get_hash(s: &str) -> Option<String> {
    let lower = s.to_lowercase();

    let mut collapsed = String::with_capacity(lower.len());
    let mut last_was_space = false;

    for ch in lower.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                collapsed.push(' ');
                last_was_space = true;
            }
        } else {
            collapsed.push(ch);
            last_was_space = false;
        }
    }

    let trimmed = collapsed.trim();

    if trimmed.is_empty() {
        return None;
    }

    let mut hasher = Hasher::new();
    hasher.update(trimmed.as_bytes());

    Some(hasher.finalize().to_string())
}

#[cfg(test)]
mod tests {
    use crate::hash::get_hash;
    use proptest::prelude::*;
    proptest! {
        #[test]
        fn test_card_parser( content in "\\PC*") {
            get_hash(&content);
        }
    }

    #[test]
    fn test_hash_punctuation() {
        // Sentence-ending space
        assert_eq!(
            get_hash("A function is continuous"),
            get_hash("A function is continuous    ")
        );

        // Negation must be preserved
        assert_ne!(
            get_hash("The limit does not exist"),
            get_hash("The limit does exist")
        );

        // Apostrophes / contractions preserve meaning
        assert_ne!(
            get_hash("The function isn't continuous"),
            get_hash("The function is continuous")
        );

        // Hyphens preserve meaning
        assert_ne!(
            get_hash("A well-defined function"),
            get_hash("A well defined function")
        );

        // Parentheses affect scope
        assert_ne!(
            get_hash("Continuous (but not differentiable)"),
            get_hash("Continuous but differentiable")
        );

        // Decimal points inside numbers matter
        assert_ne!(get_hash("The value is 3.14"), get_hash("The value is 314"));

        assert_ne!(get_hash("x+y"), get_hash("x + y"));

        // Slashes encode meaning
        assert_ne!(
            get_hash("input/output mapping"),
            get_hash("input output mapping")
        );

        // Colons introduce structure
        assert_ne!(
            get_hash("Definition: a group is a set"),
            get_hash("Definition a group is a set")
        );

        // Logical symbols must be preserved
        assert_ne!(get_hash("x != y"), get_hash("x = y"));
    }

    #[test]
    fn test_hash() {
        let a = "Hello world\n  2+2-1\n";
        let b = "hello world  2+2-1";
        let c = "  HELLO\tWORLD\t\t2+2-1  ";
        let d = "hello world 2+2-1";

        let ha = get_hash(a);
        let hb = get_hash(b);
        let hc = get_hash(c);
        let hd = get_hash(d);

        assert_eq!(ha, hb);
        assert_eq!(ha, hc);
        assert_eq!(ha, hd);

        // word order
        assert_ne!(get_hash("dog bites man"), get_hash("man bites dog"));
    }

    #[test]
    fn invalid_hash() {
        let a = "    \n\n";
        let ha = get_hash(a);
        assert!(ha.is_none());

        let a = "a an science";
        let ha = get_hash(a);
        assert!(ha.is_some());
    }
}
