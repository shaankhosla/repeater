use crate::card::{Card, CardContent};
use crate::palette::Palette;
use crate::utils::pluralize_with;

pub fn rephrase_user_prompt(cards: &[Card], total_needing: usize) -> Option<String> {
    let mut sample_question: Option<String> = None;

    for card in cards {
        if let CardContent::Basic { question, .. } = &card.content
            && sample_question.is_none()
        {
            sample_question = Some(question.clone());
            break;
        }
    }

    sample_question.map(|sample| rephrase_build_user_prompt(total_needing, &sample))
}

fn rephrase_build_user_prompt(total: usize, sample_question: &str) -> String {
    format!(
        "\n{} can rephrase {} before this drill session.\n\n{}\n{}\n",
        Palette::paint(Palette::INFO, "repeater"),
        pluralize_with("basic question", total, |n| Palette::paint(
            Palette::WARNING,
            n
        )),
        Palette::dim("Example question:"),
        sample_question
    )
}

fn cloze_build_user_prompt(total_needing: usize, card_text: &str) -> String {
    let additional_missing = total_needing.saturating_sub(1);
    let mut user_prompt = String::new();

    user_prompt.push('\n');
    user_prompt.push_str(&format!(
        "{} found {} missing bracketed deletions.",
        Palette::paint(Palette::INFO, "repeater"),
        pluralize_with("cloze card", total_needing, |n| Palette::paint(
            Palette::WARNING,
            n
        )),
    ));

    user_prompt.push_str(&format!(
        "\n\n{}\n{sample}\n",
        Palette::dim("Example needing a Cloze:"),
        sample = card_text
    ));

    let other_fragment = if additional_missing > 0 {
        format!(
            " along with {}",
            pluralize_with("other card", additional_missing, |n| Palette::paint(
                Palette::WARNING,
                n
            )),
        )
    } else {
        String::new()
    };
    user_prompt.push_str(&format!(
        "\n{} can send this text{other_fragment} to an LLM to generate a Cloze for you.\n",
        Palette::paint(Palette::INFO, "repeater"),
        other_fragment = other_fragment
    ));

    user_prompt
}

pub fn cloze_user_prompt(cards: &[Card], total_needing: usize) -> Option<String> {
    let mut sample_text: Option<String> = None;

    for card in cards {
        if let CardContent::Cloze {
            text,
            cloze_range: None,
        } = &card.content
            && sample_text.is_none()
        {
            sample_text = Some(text.clone());
            break;
        }
    }

    sample_text.map(|text| cloze_build_user_prompt(total_needing, &text))
}

#[cfg(test)]
mod tests {

    use std::path::PathBuf;

    use crate::parser::cards_from_md;

    use super::*;

    #[test]
    fn test_cloze_prompt() {
        let card_text = "the moon revolves around the earth";
        let user_prompt = cloze_build_user_prompt(1, card_text);
        assert_eq!(
            user_prompt,
            "\n\u{1b}[36mrepeater\u{1b}[0m found \u{1b}[33m1\u{1b}[0m cloze card missing bracketed deletions.\n\n\u{1b}[2mExample needing a Cloze:\u{1b}[0m\nthe moon revolves around the earth\n\n\u{1b}[36mrepeater\u{1b}[0m can send this text to an LLM to generate a Cloze for you.\n"
        );

        let user_prompt = cloze_build_user_prompt(3, card_text);
        dbg!(&user_prompt);
        assert_eq!(
            user_prompt,
            "\n\u{1b}[36mrepeater\u{1b}[0m found \u{1b}[33m3\u{1b}[0m cloze cards missing bracketed deletions.\n\n\u{1b}[2mExample needing a Cloze:\u{1b}[0m\nthe moon revolves around the earth\n\n\u{1b}[36mrepeater\u{1b}[0m can send this text along with \u{1b}[33m2\u{1b}[0m other cards to an LLM to generate a Cloze for you.\n"
        )
    }

    #[test]
    fn test_getting_samples() {
        let card_path = PathBuf::from("test_data/test.md");
        let cards = cards_from_md(&card_path).expect("should be ok");
        let user_prompt = cloze_user_prompt(&cards, 1);
        // dbg!(&user_prompt);
        assert_eq!(
            user_prompt,
            Some(
                "\n\u{1b}[36mrepeater\u{1b}[0m found \u{1b}[33m1\u{1b}[0m cloze card missing bracketed deletions.\n\n\u{1b}[2mExample needing a Cloze:\u{1b}[0m\nthe moon revolves around the earth\n\n\u{1b}[36mrepeater\u{1b}[0m can send this text to an LLM to generate a Cloze for you.\n".to_string(),
            )
        );

        let user_prompt = rephrase_user_prompt(&cards, 1);
        dbg!(&user_prompt);
        assert_eq!(
            user_prompt,Some(
    "\n\u{1b}[36mrepeater\u{1b}[0m can rephrase \u{1b}[33m1\u{1b}[0m basic question before this drill session.\n\n\u{1b}[2mExample question:\u{1b}[0m\nwhat?\n".to_string()))
    }

    #[test]
    fn test_cloze_prompt_color_formatting_numbers_only() {
        use crate::utils::strip_controls_and_escapes;

        // Test that only numbers get colored, not the phrase "other cards"
        let card_text = "sample text";
        let user_prompt_3_cards = cloze_build_user_prompt(3, card_text);
        let stripped = strip_controls_and_escapes(&user_prompt_3_cards);

        // The stripped version should have "2 other cards", not color codes around "other cards"
        assert!(stripped.contains("2 other cards"));
        assert!(user_prompt_3_cards.contains("\u{1b}[33m2\u{1b}[0m other cards"));

        // Ensure the full phrase "2 other cards" is NOT wrapped in a single color code
        assert!(!user_prompt_3_cards.contains("\u{1b}[33m2 other cards\u{1b}[0m"));
    }

    #[test]
    fn test_cloze_prompt_single_card_no_other_fragment() {
        let card_text = "sample text";
        let user_prompt = cloze_build_user_prompt(1, card_text);

        // Single card should not have "along with X other cards" text
        assert!(!user_prompt.contains("along with"));
        assert!(!user_prompt.contains("other card"));
    }

    #[test]
    fn test_cloze_prompt_plural_handling() {
        use crate::utils::strip_controls_and_escapes;

        let card_text = "sample";

        // Test singular
        let prompt_1 = cloze_build_user_prompt(1, card_text);
        let stripped_1 = strip_controls_and_escapes(&prompt_1);
        assert!(stripped_1.contains("1 cloze card missing bracketed deletions"));
        assert!(!stripped_1.contains("cloze cards missing")); // Should not have plural

        // Test plural
        let prompt_2 = cloze_build_user_prompt(2, card_text);
        let stripped_2 = strip_controls_and_escapes(&prompt_2);
        assert!(stripped_2.contains("2 cloze cards missing bracketed deletions"));
        assert!(stripped_2.contains("1 other card")); // 2-1 = 1 other card (singular)

        // Test plural with multiple others
        let prompt_5 = cloze_build_user_prompt(5, card_text);
        let stripped_5 = strip_controls_and_escapes(&prompt_5);
        assert!(stripped_5.contains("5 cloze cards missing bracketed deletions"));
        assert!(stripped_5.contains("4 other cards")); // 5-1 = 4 other cards (plural)
    }

    #[test]
    fn test_rephrase_prompt_formatting() {
        use crate::utils::strip_controls_and_escapes;

        let prompt = rephrase_build_user_prompt(3, "What is the capital?");
        let stripped = strip_controls_and_escapes(&prompt);

        // Verify numbers are highlighted
        assert!(prompt.contains("\u{1b}[33m3\u{1b}[0m"));

        // Verify "repeater" is highlighted
        assert!(prompt.contains("\u{1b}[36mrepeater\u{1b}[0m"));

        // Verify stripped version has expected text
        assert!(stripped.contains("3 basic question"));
        assert!(stripped.contains("What is the capital?"));
    }
}
