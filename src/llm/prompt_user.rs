use crate::card::{Card, CardContent};
use crate::palette::Palette;

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
    let plural = if total == 1 { "" } else { "s" };
    format!(
        "\n{} can rephrase {} basic question{plural} before this drill session.\n\n{}\n{}\n",
        Palette::paint(Palette::INFO, "repeater"),
        Palette::paint(Palette::WARNING, total),
        Palette::dim("Example question:"),
        sample_question
    )
}

fn cloze_build_user_prompt(total_needing: usize, card_text: &str) -> String {
    let additional_missing = total_needing.saturating_sub(1);
    let mut user_prompt = String::new();

    let plural = if total_needing == 1 { "" } else { "s" };

    user_prompt.push('\n');
    user_prompt.push_str(&format!(
        "{} found {} cloze card{plural} missing bracketed deletions.",
        Palette::paint(Palette::INFO, "repeater"),
        Palette::paint(Palette::WARNING, total_needing),
        plural = plural,
    ));

    user_prompt.push_str(&format!(
        "\n\n{}\n{sample}\n",
        Palette::dim("Example needing a Cloze:"),
        sample = card_text
    ));

    let other_fragment = if additional_missing > 0 {
        let other_plural = if additional_missing == 1 { "" } else { "s" };
        format!(
            " along with {} other card{other_plural}",
            Palette::paint(Palette::WARNING, additional_missing),
            other_plural = other_plural
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
}
