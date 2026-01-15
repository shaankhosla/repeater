use std::sync::Arc;

use anyhow::{Context, Result};
use async_openai::Client;
use async_openai::config::OpenAIConfig;

use crate::card::{Card, CardContent};
use crate::cloze_utils::cloze_user_prompt;
use crate::question_utils::rephrase_user_prompt;

use super::ensure_client;

const MAX_CONCURRENT_LLM_REQUESTS: usize = 4;

#[derive(Clone, Debug)]
pub struct DrillPreprocessor {
    client: Option<Arc<Client<OpenAIConfig>>>,
    rephrase_questions: bool,
    cards_needing_clozes: usize,
    cards_needing_rephrase: usize,
}

impl DrillPreprocessor {
    pub fn new(cards: &[Card], rephrase_questions: bool) -> Result<Self> {
        let cards_needing_clozes = count_cards_needing_clozes(cards);
        let cards_needing_rephrase = count_cards_neeing_rephrase(cards, rephrase_questions);

        let client = if cards_needing_rephrase > 0 {
    rephrase_user_prompt(cards).map(|prompt| {
        ensure_client(&prompt)
            .with_context(|| "Failed to initialize LLM client, cannot rephrase {cards_needing_rephrase} questions")
            .map(Arc::new)
    })
} else if cards_needing_clozes > 0 {
    cloze_user_prompt(cards).map(|prompt| {

        ensure_client(&prompt)
            .with_context(|| {
                "Failed to initialize LLM client, cannot synthesize Cloze deletions for {cards_needing_clozes} Cloze cards without brackets".to_string()
            })
            .map(Arc::new)
    })
} else {
    None
}
.transpose()?;

        Ok(Self {
            client,
            rephrase_questions,
            cards_needing_clozes,
            cards_needing_rephrase,
        })
    }

    pub fn llm_required(&self) -> bool {
        self.client.is_some()
    }
}

fn count_cards_needing_clozes(cards: &[Card]) -> usize {
    cards
        .iter()
        .filter(|card| {
            matches!(
                card.content,
                CardContent::Cloze {
                    cloze_range: None,
                    ..
                }
            )
        })
        .count()
}

fn count_cards_neeing_rephrase(cards: &[Card], rephrase_questions: bool) -> usize {
    if !rephrase_questions {
        return 0_usize;
    }

    cards
        .iter()
        .filter(|card| matches!(card.content, CardContent::Basic { .. }))
        .count()
}
