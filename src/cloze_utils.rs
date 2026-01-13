use crate::card::{Card, CardContent, ClozeRange};
use crate::llm::{ensure_client, request_cloze};
use crate::palette::Palette;
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use futures::stream::{self, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};

const MAX_CONCURRENT_LLM_REQUESTS: usize = 4;

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

fn build_user_prompt(total_missing: usize, card_text: &str) -> String {
    let additional_missing = total_missing.saturating_sub(1);
    let mut user_prompt = String::new();

    let plural = if total_missing == 1 { "" } else { "s" };

    user_prompt.push('\n');
    user_prompt.push_str(&format!(
        "{} found {} cloze card{plural} missing bracketed deletions.",
        Palette::paint(Palette::INFO, "repeater"),
        Palette::paint(Palette::WARNING, total_missing),
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

async fn replace_missing_clozes(
    cards: &mut [Card],
    cards_with_no_clozes: Vec<(String, String)>,
    index_by_hash: &HashMap<String, usize>,
    client: Arc<Client<OpenAIConfig>>,
) -> Result<()> {
    let mut tasks = stream::iter(cards_with_no_clozes.into_iter().map(|(hash, text)| {
        let client = Arc::clone(&client);
        async move {
            let new_cloze_text = request_cloze(&client, &text).await.with_context(|| {
                format!("Failed to synthesize cloze text for card:\n\n{}", text)
            })?;
            Ok::<_, anyhow::Error>((hash, new_cloze_text))
        }
    }))
    .buffer_unordered(MAX_CONCURRENT_LLM_REQUESTS);
    while let Some(llm_output) = tasks.next().await {
        let (hash, new_cloze_text) = llm_output?;

        let Some(&idx) = index_by_hash.get(&hash) else {
            continue;
        };
        let card = &mut cards[idx];
        if let CardContent::Cloze {
            text, cloze_range, ..
        } = &mut card.content
        {
            let cloze_idxs = find_cloze_ranges(&new_cloze_text);
            let new_cloze_range: ClozeRange = cloze_idxs
                .first()
                .map(|(start, end)| ClozeRange::new(*start, *end))
                .transpose()?
                .ok_or_else(|| {
                    anyhow::anyhow!("No cloze range found. LLM output: {new_cloze_text}")
                })?;
            *cloze_range = Some(new_cloze_range);
            *text = new_cloze_text;
        }
    }

    Ok(())
}

pub async fn resolve_missing_clozes(cards: &mut [Card]) -> Result<()> {
    let cards_with_no_clozes: Vec<_> = cards
        .iter()
        .filter_map(|card| {
            if let CardContent::Cloze {
                text,
                cloze_range: None,
            } = &card.content
            {
                Some((card.card_hash.clone(), text.clone()))
            } else {
                None
            }
        })
        .collect();

    if cards_with_no_clozes.is_empty() {
        return Ok(());
    }
    let total_missing = cards_with_no_clozes.len();
    let card_text = &cards_with_no_clozes[0].1;
    let user_prompt = build_user_prompt(total_missing, card_text);

    let index_by_hash: HashMap<String, usize> = cards
        .iter()
        .enumerate()
        .map(|(i, c)| (c.card_hash.clone(), i))
        .collect();

    let plural = if total_missing == 1 { "" } else { "s" };
    let client = ensure_client(&user_prompt)
       .with_context(|| format!("Failed to initialize LLM client, cannot synthesize Cloze text for {} card{plural} in your collection", total_missing))?;
    let client = Arc::new(client);

    replace_missing_clozes(cards, cards_with_no_clozes, &index_by_hash, client).await?;

    Ok(())
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

    #[test]
    fn test_user_prompt() {
        let card_text = "the moon revolves around the earth";
        let user_prompt = build_user_prompt(1, card_text);
        assert_eq!(
            user_prompt,
            "\n\u{1b}[36mrepeater\u{1b}[0m found \u{1b}[33m1\u{1b}[0m cloze card missing bracketed deletions.\n\n\u{1b}[2mExample needing a Cloze:\u{1b}[0m\nthe moon revolves around the earth\n\n\u{1b}[36mrepeater\u{1b}[0m can send this text to an LLM to generate a Cloze for you.\n"
        );

        let user_prompt = build_user_prompt(3, card_text);
        dbg!(&user_prompt);
        assert_eq!(
            user_prompt,
            "\n\u{1b}[36mrepeater\u{1b}[0m found \u{1b}[33m3\u{1b}[0m cloze cards missing bracketed deletions.\n\n\u{1b}[2mExample needing a Cloze:\u{1b}[0m\nthe moon revolves around the earth\n\n\u{1b}[36mrepeater\u{1b}[0m can send this text along with \u{1b}[33m2\u{1b}[0m other cards to an LLM to generate a Cloze for you.\n"
        )
    }
}
