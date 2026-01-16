use crate::card::ClozeRange;

pub fn find_cloze_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = None;

    for (i, ch) in text.char_indices() {
        match ch {
            '[' if start.is_none() => start = Some(i),
            ']' => {
                if let Some(s) = start.take() {
                    let e = i + ch.len_utf8();
                    ranges.push((s, e));
                }
            }
            _ => {}
        }
    }

    ranges
}

pub fn mask_cloze_text(text: &str, range: &ClozeRange) -> String {
    let start = range.start;
    let end = range.end;
    let hidden_section = &text[start..end];
    let core = hidden_section.trim_start_matches('[').trim_end_matches(']');
    let placeholder = "_".repeat(core.chars().count().max(3));

    let masked = format!("{}[{}]{}", &text[..start], placeholder, &text[end..]);
    masked
}

#[cfg(test)]
mod tests {
    use crate::card::ClozeRange;
    use crate::cloze_utils::find_cloze_ranges;

    use super::*;
    #[test]
    fn mask_cloze_text_handles_unicode_and_bad_ranges() {
        let text = "Capital of 日本 is [東京]";

        let cloze_idxs = find_cloze_ranges(text);
        let range: ClozeRange = cloze_idxs
            .first()
            .map(|(start, end)| ClozeRange::new(*start, *end))
            .transpose()
            .unwrap()
            .unwrap();
        let masked = mask_cloze_text(text, &range);
        assert_eq!(masked, "Capital of 日本 is [___]");

        let text = "Capital of 日本 is [longer text is in this bracket]";

        let cloze_idxs = find_cloze_ranges(text);
        let range: ClozeRange = cloze_idxs
            .first()
            .map(|(start, end)| ClozeRange::new(*start, *end))
            .transpose()
            .unwrap()
            .unwrap();
        let masked = mask_cloze_text(text, &range);
        assert_eq!(
            masked,
            "Capital of 日本 is [______________________________]"
        );
    }
}
